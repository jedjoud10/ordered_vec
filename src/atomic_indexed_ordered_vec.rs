use std::{
    fmt::Debug,
    ops::{Index, IndexMut}, sync::{atomic::{AtomicUsize, Ordering::Relaxed}, RwLock, Arc, mpsc::{Sender, Receiver}},
};

/// A collection that keeps the ordering of its elements, even when deleting an element
/// However, this collection can be shared between threads
/// We can only add elements in those threads for now, we still must delete the elements on the main thread
pub struct AtomicIndexedOrderedVec<T> {
    /// A list of the current elements in the list
    pub(crate) vec: Option<Vec<Option<T>>>,
    /// A list of the indices that contain a null element, so whenever we add a new element, we will add it there
    pub(crate) missing: Arc<RwLock<Vec<usize>>>,
    /// A counter that increases every time we add an element to the list in other threads, before the main update
    counter: AtomicUsize,
    /// The current length of the vector 
    length: AtomicUsize,
    /// The thread on which we created this ordered vec
    thread_id: std::thread::ThreadId,
    /// Are we on the creation thread?
    creation_thread: bool,
    /// Some messaging stuff used to tell the creation thread where and what elements we want to insert
    tx: Sender<(usize, T)>,
    rx: Option<Receiver<(usize, T)>>,
}

impl<T> Default for AtomicIndexedOrderedVec<T> {
    fn default() -> Self {
        // Create the channel
        let (tx, rx) = std::sync::mpsc::channel::<(usize, T)>();
        Self {
            vec: Some(Vec::new()),
            missing: Arc::new(RwLock::new(Vec::new())),
            counter: AtomicUsize::new(0),
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
    pub fn push_shove(&mut self, elem: T) -> usize {
        // Check if we are on the creation thread
        if self.creation_thread {
            // Do this normally
            if self.missing.as_ref().read().unwrap().is_empty() {
                // Add the element normally
                self.vec.as_mut().unwrap().push(Some(elem));
                return self.vec.as_ref().unwrap().len() - 1;
            } else {
                // If we have some null elements, we can validate the given element there
                let mut write = self.missing.as_ref().write().unwrap();
                let vec = self.vec.as_mut().unwrap();
                let idx = write.pop().unwrap();
                *vec.get_mut(idx).unwrap() = Some(elem);
                return idx;
            }
        } else {
            // Multi-threaded way
            let read = self.missing.as_ref().read().unwrap();
            let ctr = self.counter.fetch_add(1, Relaxed);
            read.get(ctr).cloned().unwrap_or(self.length.load(Relaxed))
        }       
    }
    /// Get the index of the next element that we will add
    pub fn get_next_idx(&self) -> usize {
        // Normal push
        if self.missing.is_empty() {
            return self.vec.len();
        }
        // Shove
        *self.missing.last().unwrap()
    }
    /// Remove an element that was already added
    pub fn remove(&mut self, idx: usize) -> Option<T> {
        self.missing.push(idx);
        let elem = self.vec.get_mut(idx)?;
        let elem = std::mem::take(elem);
        elem
    }
    /// Get a reference to an element in the ordered vector
    pub fn get(&self, idx: usize) -> Option<&T> {
        self.vec.get(idx)?.as_ref()
    }
    /// Get a mutable reference to an element in the ordered vector
    pub fn get_mut(&mut self, idx: usize) -> Option<&mut T> {
        self.vec.get_mut(idx)?.as_mut()
    }
    /// Get the number of valid elements in the ordered vector
    pub fn count(&self) -> usize {
        self.vec.len() - self.missing.len()
    }
    /// Get the number of invalid elements in the ordered vector
    pub fn count_invalid(&self) -> usize {
        self.missing.len()
    }
    /// Clear the whole ordered vector
    pub fn clear(&mut self) -> Vec<Option<T>> {
        let len = self.vec.len();
        self.missing = (0..len).collect::<Vec<_>>();
        // https://users.rust-lang.org/t/how-to-initialize-vec-option-t-with-none/30580
        let empty = std::iter::repeat_with(|| None)
            .take(len)
            .collect::<Vec<_>>();

        let cleared = std::mem::replace(&mut self.vec, empty);
        cleared
    }
}

/// Iter magic
impl<T> OrderedVec<T> {
    /// Get an iterator over the valid elements
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.vec.iter().filter_map(|val| val.as_ref())
    }
    /// Get an iterator over the valid elements
    pub fn iter_indexed(&self) -> impl Iterator<Item = (usize, &T)> {
        self.vec
            .iter()
            .enumerate()
            .filter_map(|(index, val)| match val {
                Some(val) => Some((index, val)),
                None => None,
            })
    }
    /// Get a mutable iterator over the valid elements
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.vec.iter_mut().filter_map(|val| val.as_mut())
    }
    /// Get a mutable iterator over the valid elements with their index
    pub fn iter_indexed_mut(&mut self) -> impl Iterator<Item = (usize, &mut T)> {
        self.vec
            .iter_mut()
            .enumerate()
            .filter_map(|(index, val)| match val {
                Some(val) => Some((index, val)),
                None => None,
            })
    }
    /// Get an iterator over the indices of the null elements
    pub fn iter_invalid(&self) -> impl Iterator<Item = &usize> {
        self.missing.iter()
    }
    /// Drain the elements that only return true. This will return just an Iterator of the index and value of the drained elements
    pub fn my_drain<F>(&mut self, mut filter: F) -> impl Iterator<Item = (usize, T)> + '_
    where
        F: FnMut(usize, &T) -> bool,
    {
        // Keep track of which elements should be removed
        let indices = self
            .iter_indexed()
            .filter_map(|(index, val)| {
                if filter(index, val) {
                    Some(index)
                } else {
                    None
                }
            })
            .collect::<Vec<usize>>();
        // Now actually remove them
        indices
            .into_iter()
            .map(|idx| (idx, self.remove(idx).unwrap()))
    }
}

/// Traits
impl<T> Index<usize> for OrderedVec<T> {
    type Output = T;
    fn index(&self, index: usize) -> &Self::Output {
        self.get(index).unwrap()
    }
}

impl<T> IndexMut<usize> for OrderedVec<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.get_mut(index).unwrap()
    }
}
