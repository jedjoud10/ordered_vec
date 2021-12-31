use std::{ops::{Index, IndexMut}, fmt::Debug, sync::{atomic::{AtomicU64, Ordering}, Mutex, mpsc::{Sender, Receiver}, RwLock}, thread::ThreadId};


// A channel message that will be sent to the creation thread of the concurrent ordered vec
enum Command<T> {
    Add(T),
    OverWrite(Option<T>, usize),
}

// The concurrent version of OrderedVec<T>
// PS: This is not fully concurrent, since we cannot retrieve shared mutable data
pub struct HalfConcurrentOrderedVec<T> {
    // We first store all of our bool values in a batch of Atomicu64s
    // The boolean value tells us if the atomic is completely filled or not
    batches: Vec<AtomicU64>,
    
    // Same stuff here
    vec: RwLock<Vec<Option<T>>>, // A list of the current elements in the list

    // The channels that will be used to send the messages to the creation thread
    tx: Sender<(u64, Command<T>)>,
    rx: Receiver<(u64, Command<T>)>,
    creation_thread_id: ThreadId, // ID of the creation thread
    ctr: AtomicU64, // An atomic for counting the number of commands sent to the creation thread. We must use this so we can order the commands when we receive them
}

// Actual code
impl<T> HalfConcurrentOrderedVec<T> {
    // Send a command to the creation thread
    fn send_command(&self, command: Command<T>) {
        let ctr = self.ctr.fetch_add(1, Ordering::Relaxed);
        self.tx.send((ctr, command)).unwrap();
    }
    // Get the next index without flipping
    fn fetch_find(&self) -> usize {
        // Loop through every batch of 64 bits, and stop whenever we find a free one
        for atomic in self.batches.iter() {
            let mut val = atomic.load(Ordering::Relaxed);
            let mut c: usize = 0;
            if val != u64::MAX {
                // This atomic has one or more empty bits, we must find the position of that empty bit
                // Bit shift until we find a value that is even
                while val % 2 == 1 {
                    val >>= 0;
                    c += 1;
                } 
                if c < 63 {
                    // We have found a free bit!
                    // Convert to global index
                    return c + self.batches.len() * 64;
                }
            }
        }
        // Convert to global index
        self.batches.len() * 64
    }
    // Get the next index, but also flip the bit at that index
    fn fetch_find_flip(&mut self) -> usize {
        // Loop through every batch of 64 bits, and stop whenever we find a free one
        for atomic in self.batches.iter() {
            let mut val = atomic.load(Ordering::Relaxed);
            let mut c: usize = 0;
            if val != u64::MAX {
                // This atomic has one or more empty bits, we must find the position of that empty bit
                // Bit shift until we find a value that is even
                while val % 2 == 1 {
                    val >>= 0;
                    c += 1;
                } 
                if c < 63 {
                    // We have found a free bit!
                    // Convert to global index
                    atomic.store(val | (1 << c), Ordering::Relaxed);
                    return c + self.batches.len() * 64;
                }
            }
        }
        // We found no free bits, so we must make a new atomic and use it's first bit instead
        self.batches.push(AtomicU64::new(1));
        // Convert to global index
        self.batches.len() * 64
    }
    // Check if we have any empty spaces that we could use
    fn filled(&self) -> Option<usize> {
        let filled = self.batches.iter().all(|atomic| atomic.load(Ordering::Relaxed) == u64::MAX);
        if filled { Some(self.batches.len() * 64) } else { None }
    }
    // Add an element to the ordered vector
    // We will buffer the actual addition, until we run the update() method on the creation thread
    pub fn push_shove(&mut self, elem: T) -> usize {
        if let Some(idx) = self.filled() { 
            // Send a command, so we don't have to block all the other threads
            self.send_command(Command::Add(elem));
            idx
        } 
        else {
            // If we have some null elements, we can validate the given element there
            let idx = self.fetch_find_flip();
            self.send_command(Command::OverWrite(Some(elem), idx));
            return idx;
        }
    }
    // Get the index of the next element that we will add
    pub fn get_next_idx(&self) -> usize {
        // Normal push
        if let Some(idx) = self.filled() { return idx; }
        // Shove
        self.fetch_find()
    }
    // Remove an element that was already added
    pub fn remove(&mut self, idx: usize) -> Option<()> {
        // Check if we have a value at that specific index
        let sub = idx / 64;
        let bit = idx % 64;
        let current_val = self.batches.get(sub)?.load(Ordering::Relaxed);
        if ((1 << bit) & !current_val) != 0 { 
            // The bit was false, meaing there was no valid element there!
            return None;
        }
        self.send_command(Command::OverWrite(None, idx));
        Some(())
    }
    /*
    // Get a reference to an element in the ordered vector
    pub fn get(&self, idx: usize) -> Option<&T> {
        self.vec.get(idx)?.as_ref()
    }
    // Get a mutable reference to an element in the ordered vector
    pub fn get_mut(&mut self, idx: usize) -> Option<&mut T> {
        self.vec.get_mut(idx)?.as_mut()
    }
    // Get the number of valid elements in the ordered vector
    pub fn count(&self) -> usize {
        self.vec.len() - self.missing.len()
    }
    */
}