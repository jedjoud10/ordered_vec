use std::{
    fmt::Debug,
    ops::{Index, IndexMut}, sync::{atomic::{AtomicUsize, Ordering::Relaxed, AtomicU64}, RwLock, Arc, mpsc::{Sender, Receiver}},
};
use bitfield::AtomicSparseBitfield;

use super::{command::AtomicIndexedCommand, message::AtomicIndexedMessageType};

/// A collection that keeps the ordering of its elements, even when deleting an element
/// However, this collection can be shared between threads
/// We can add and remove elements from other threads
pub struct AtomicIndexedOrderedVec<T> {
    /// A list of the current elements in the list
    pub(crate) vec: RwLock<Vec<Option<T>>>,
    /// A list of the indices that contain a null element, so whenever we add a new element, we will add it there
    pub(crate) missing: RwLock<Vec<usize>>,
    /// A counter that increases every time we add an element to the list in other threads, before the main update
    counter: AtomicUsize,
    /// The current length of the vector 
    length: AtomicUsize,
    /// The amount of commands that we have sent during this "frame"
    command_counter: AtomicUsize,
    /// Keep count of the number of "empty" cells
    empty_count: AtomicUsize,
    /// An atomic sparse bitfield used to tell the state of each element index. It can either be "valid" or "empty"
    bitfield: AtomicSparseBitfield,
    /// The thread on which we created this ordered vec
    thread_id: std::thread::ThreadId,
    /// Are we on the creation thread?
    creation_thread: bool,
    /// Some messaging stuff used to send commands to the creation thread
    tx: Sender<AtomicIndexedCommand<T>>,
    rx: Option<Receiver<AtomicIndexedCommand<T>>>,
}

impl<T> Default for AtomicIndexedOrderedVec<T> {
    fn default() -> Self {
        // Create the channel
        let (tx, rx) = std::sync::mpsc::channel::<AtomicIndexedCommand<T>>();
        Self {
            vec: RwLock::new(Vec::new()),
            missing: RwLock::new(Vec::new()),
            counter: AtomicUsize::new(0),
            command_counter: AtomicUsize::new(0),
            empty_count: AtomicUsize::new(0),
            bitfield: AtomicSparseBitfield::new(),
            length: AtomicUsize::new(0),
            thread_id: std::thread::current().id(),
            creation_thread: true,
            tx,
            rx: Some(rx),
        }
    }
}
/// Actual code
impl<T> AtomicIndexedOrderedVec<T> {
    /// Add an element to the ordered vector
    /// This will send a message to the "creation thread", but it will also return the proper index
    pub fn push_shove(&self, elem: T) -> usize {
        // Check if we are on the creation thread
        let idx = if self.creation_thread {
            // Do this normally
            if self.missing.read().unwrap().is_empty() {
                // Add the element normally
                let mut vec = self.vec.write().unwrap();
                vec.push(Some(elem));
                let idx = vec.len() - 1;
                // Update the bitfield, since this cell has become "valid"
                self.bitfield.set(idx as u64, true);
                return idx;
            } else {
                // If we have some null elements, we can validate the given element there
                let mut write = self.missing.write().unwrap();
                let mut vec = self.vec.write().unwrap();
                let idx = write.pop().unwrap();
                *vec.get_mut(idx).unwrap() = Some(elem);
                // Update the bitfield, since this cell has become "valid"
                self.bitfield.set(idx as u64, true);
                return idx;
            }
        } else {
            // Multi-threaded way
            let read = self.missing.read().unwrap();
            let ctr = self.counter.fetch_add(1, Relaxed);
            let idx = read.get(ctr).cloned().unwrap_or_else(|| self.length.fetch_add(1, Relaxed));   
            // Send a message saying that we must add the element here
            self.tx.send(AtomicIndexedCommand::new(self.command_counter.fetch_add(1, Relaxed), AtomicIndexedMessageType::Add(elem, idx))).unwrap();   
            // If the current cell is empty, that means that we will be replacing the cell with this item, so update the empty counter
            self.empty_count.fetch_sub(1, Relaxed);
            idx
        };
        // Update the bitfield, since this cell has become "valid"
        self.bitfield.set(idx as u64, true);      
        idx
    }
    /// Get the index of the next element that we will add
    pub fn get_next_idx(&self) -> usize {
        // Check if we are on the creation thread
        if self.creation_thread {
            // Do this normally
            let read = self.missing.read().unwrap();
            // Normal push
            if read.is_empty() {
                let vec = self.vec.read().unwrap();
                return vec.len();
            }
            // Get ID
            *read.last().unwrap()
        } else {
            // Multi-threaded way
            let read = self.missing.read().unwrap();
            let ctr = self.counter.load(Relaxed);
            let idx = read.get(ctr).cloned().unwrap_or_else(|| self.length.load(Relaxed));
            idx
        }        
    }
    /// Remove an element that was already added
    /// This will send a message to the creation thread telling us that we must remove an element at a specific index
    /// If we remove an element on ThreadA, and we try to add an element on ThreadB, the two elements will have different IDs, even though they should have the same ID.
    pub fn remove(&self, idx: usize) -> Option<()> {
        // Check if we are on the creation thread
        if self.creation_thread {
            let mut write = self.missing.write().ok()?;
            write.push(idx);
        } else {
            // Multi-threaded way
            // Check if the element at the index is actually valid, because if it is not, we have a problem
            if self.bitfield.get(idx as u64) {
                // The cell is filled, we can safely remove the element
                self.tx.send(AtomicIndexedCommand::new(self.command_counter.fetch_add(1, Relaxed), AtomicIndexedMessageType::Remove(idx))).unwrap();                
            } else {
                // The cell is empty, we have a problemo
                return None;
            }
        };

        // Update the bitfield if it came back valid
        self.empty_count.fetch_add(1, Relaxed);
        self.bitfield.set(idx as u64, false);
        Some(())
    }
    /// Get the number of valid elements in the ordered vector
    /// We must take the atomics in consideration here
    pub fn count(&self) -> usize {
        self.length.load(Relaxed) - self.empty_count.load(Relaxed)
    }
    /// Get the number of invalid elements in the ordered vector
    pub fn count_invalid(&self) -> usize {
        self.empty_count.load(Relaxed)
    }
    /// Update the atomic indexed ordered vec by reading all the commands, reseting the atomics, and applying the commands
    /// This must be ran on the creation thread
    pub fn update(&self) {
        // Read all the commands and wait for them
        let mut command_count = self.command_counter.load(Relaxed);
        // Reset the atomics
        self.command_counter.store(0, Relaxed);
        self.counter.store(0, Relaxed);
        // Wait for the commands now
        let mut cbuffer: Vec<AtomicIndexedCommand<T>> = Vec::new(); 
        while command_count > 0 {
            if let Ok(x) = self.rx.as_ref().unwrap().recv() {
                // Take the command and buffer it
                command_count -= 1;
                cbuffer.push(x);
            }
        }
        // Sort the commands
        cbuffer.sort_by(|a, b|  usize::cmp(&a.command_id, &b.command_id));

        // Apply the commands
        for command in cbuffer {
            match command.message {
                AtomicIndexedMessageType::Add(elem, id) => { self.push_shove(elem); },
                AtomicIndexedMessageType::Remove(id) => { self.remove(id); },
            }
        }
        let vec = self.vec.read().unwrap();
        self.length.store(vec.len(), Relaxed);
    } 
}
