use std::{
    fmt::{Debug},
    marker::PhantomData,
    ops::{Index, IndexMut},
    sync::{
        atomic::{AtomicUsize, Ordering::Relaxed},
        Arc, RwLock,
    },
};

use crate::simple::OrderedVec;


/// Some sort of shareable index, that might contain the index of the missing index.
/// Yea, pretty confusing
#[derive(Debug)]
pub struct ShareableIndex {
    /// The actual index of the element. This is just a wrapper around it
    pub(crate) index: usize,
    // If we have replaced an empty cell with the current index's element, this will be that cell's index in the "missing" vector
    pub(crate) missing_indices_idx: Option<usize>,
}

impl std::fmt::Display for ShareableIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.index)
    }
}

impl ShareableIndex {
    /// Create a new shareable index using a normal index and a missing index index
    pub(crate) fn new(index: usize, missing_indices_idx: Option<usize>) -> Self {
        Self {
            index, missing_indices_idx
        }
    }
}

/// A collection that keeps the ordering of its elements, even when deleting an element
/// However, this collection can be shared between threads
/// We can *guess* what the index is for an element that we must add
/// We can use **get**, and **get_next_id_increment** on other threads, but that is all
/// We must do the rest of our operations using an external messaging system
pub struct ShareableOrderedVec<T> {
    /// A list of the current elements in the list
    pub(crate) vec: Vec<Option<T>>,
    /// A list of the indices that contain a null element, so whenever we add a new element, we will add it there
    pub(crate) missing: Arc<RwLock<OrderedVec<usize>>>,
    /// A counter that increases every time we add an element to the list in other threads, before the main update
    pub(crate) counter: Arc<AtomicUsize>,
    /// The current length of the vector. This will increase when we add an elements that is outisde of the current vector
    pub(crate) length: Arc<AtomicUsize>,
}

impl<T> Default for ShareableOrderedVec<T> {
    fn default() -> Self {
        Self {
            vec: Vec::new(),
            missing: Arc::new(RwLock::new(OrderedVec::new())),
            counter: Arc::new(AtomicUsize::new(0)),
            length: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl<T> Debug for ShareableOrderedVec<T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShareableOrderedVec")
            .field("vec", &self.vec)
            .field("missing", &self.missing.as_ref().read().unwrap())
            .finish()
    }
}

/// Actual code that we will update on the main thread, or the creation thread
impl<T> ShareableOrderedVec<T> {
    /// Add an element to the ordered vector, but at a specific location
    /// This will return the last element that was at that position, if possible
    pub fn insert(&mut self, idx: ShareableIndex, elem: T) -> Option<T> {
        // Check the length first
        dbg!(&idx);
        let missing_indices_idx = idx.missing_indices_idx;
        let idx = idx.index;
        if idx >= self.vec.len() {
            // All the empty elements that we will add are pretty much considered empty elements, so we need to update that while adding them
            let mut missing = self.missing.write().unwrap();
            let mut old_len = self.vec.len();
            // We must resize and add
            self.vec.resize_with(idx, || {
                // Add the index for the empty value
                missing.push_shove(old_len);
                old_len += 1;
                // We want to fill the gap with just empty values
                None
            });
            // Actually insert the elements
            self.vec.push(Some(elem));
            if self.vec.len() - 1 != idx {
                panic!()
            }
            return None;
        } else {
            // This can either be an overwrite to a missing cell or a write to empty cells that were created using the resize
            if let Some(missing_indices_idx) = missing_indices_idx {
                // This is an overwrite to an empty cell
            } else {
                // This is an overwrite to a cell that isn't actually considered empty, but it actually is
            }
            // If we have an element there, we also panic
            if self.vec.get(idx).unwrap().is_some() {
                panic!()
            }
            // Replace
            let dest = self.vec.get_mut(idx).unwrap();
            let last = std::mem::replace(dest, Some(elem));
            // We have replaced an empty slot, so we must update our missing cells
            let mut missing = self.missing.write().unwrap();
            // We are fine since we remove the elements starting from the end of the list anyways
            missing.remove(missing_indices_idx.unwrap());
            return last;
        }
    }
    /// Get the index of the next element that we will add. If we call this twice, without inserting any elements, it will not change
    pub fn get_next_idx(&self) -> usize {
        // Normal push
        let missing = self.missing.read().unwrap();
        missing.iter().last().cloned().unwrap_or(self.vec.len())
    }
    /// Check the next ID where we can add an element, but also increment the counter, so it won't be the same ID
    pub fn get_next_id_increment(&self) -> ShareableIndex {
        // Try to get an empty cell, if we couldn't just use the length as the index
        let missing = self.missing.as_ref().read().unwrap();
        let ctr = self.counter.fetch_add(1, Relaxed);
        let missing_idx = missing.count().checked_sub(ctr);
        if let Some(Some(&idx)) = missing_idx.map(|x| missing.get(x)) {
            // We must replace an already existing "empty" cell
            ShareableIndex::new(idx, Some(missing.count() - ctr))
        } else {
            // We must add the cell at the end of the list
            let len = self.length.fetch_add(1, Relaxed);
            ShareableIndex::new(len, None)
        }
    }
    /// Remove an element that was already added
    pub fn remove(&mut self, idx: usize) -> Option<T> {
        let mut missing = self.missing.write().unwrap();
        missing.push_shove(idx);
        let elem = self.vec.get_mut(idx)?;
        let elem = std::mem::take(elem);
        elem
    }
    /// Update the atomic counters at the start, before we do anything on the other threads.
    pub fn init_update(&mut self) {
        self.counter.store(0, Relaxed);
        self.length.store(self.vec.len(), Relaxed);
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
        let missing = self.missing.read().unwrap();
        self.vec.len() - missing.count()
    }
    /// Get the number of invalid elements in the ordered vector
    pub fn count_invalid(&self) -> usize {
        let missing = self.missing.read().unwrap();
        missing.count()
    }
    /// Clear the whole ordered vector
    pub fn clear(&mut self) -> Vec<Option<T>> {
        let len = self.vec.len();
        let mut missing = self.missing.write().unwrap();
        *missing = OrderedVec::from_valids((0..len).collect::<Vec<_>>());
        // https://users.rust-lang.org/t/how-to-initialize-vec-option-t-with-none/30580
        let empty = std::iter::repeat_with(|| None)
            .take(len)
            .collect::<Vec<_>>();

        let cleared = std::mem::replace(&mut self.vec, empty);
        cleared
    }
}

/// Iter magic
impl<T> ShareableOrderedVec<T> {
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
    pub fn iter_invalid(&self) -> impl Iterator<Item = usize> {
        let missing = self.missing.as_ref().write().unwrap().clone();
        missing.into_iter()
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
impl<T> Index<usize> for ShareableOrderedVec<T> {
    type Output = T;
    fn index(&self, index: usize) -> &Self::Output {
        self.get(index).unwrap()
    }
}

impl<T> IndexMut<usize> for ShareableOrderedVec<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.get_mut(index).unwrap()
    }
}
