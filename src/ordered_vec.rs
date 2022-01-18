use std::{
    fmt::Debug,
    ops::{Index, IndexMut},
};

use crate::utils::{to_id, IndexPair, from_id};

/// A collection that keeps the ordering of its elements, even when deleting an element
/// This also supports versioning, so if we add two elements and they have the same physical index, they will not have the same ID
/// https://www.david-colson.com/2020/02/09/making-a-simple-ecs.html
pub struct OrderedVec<T> {
    /// A list of the current elements in the list
    pub(crate) vec: Vec<(Option<T>, u32)>,
    /// A list of the indices that contain a null element, so whenever we add a new element, we will add it there
    pub(crate) missing: Vec<u64>,
}

impl<T> Clone for OrderedVec<T>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        Self {
            vec: self.vec.clone(),
            missing: self.missing.clone(),
        }
    }
}

impl<T> Debug for OrderedVec<T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OrderedVec")
            .field("vec", &self.vec)
            .field("missing", &self.missing)
            .finish()
    }
}

impl<T> Default for OrderedVec<T> {
    fn default() -> Self {
        Self {
            vec: Vec::new(),
            missing: Vec::new(),
        }
    }
}

/// Actual code
impl<T> OrderedVec<T> {
    /// New
    pub fn new() -> Self {
        Self::default()
    }
    /// Create Self using already existing elements
    pub fn from_valids(vals: Vec<T>) -> Self {
        Self {
            vec: vals
                .into_iter()
                .map(|x| (Some(x), 0))
                .collect::<Vec<(Option<T>, u32)>>(),
            missing: Vec::new(),
        }
    }
    /// Add an element to the ordered vector
    pub fn push_shove(&mut self, elem: T) -> u64 {
        if self.missing.is_empty() {
            // Add the element normally
            self.vec.push((Some(elem), 0));
            (self.vec.len() - 1) as u64
        } else {
            // If we have some null elements, we can validate the given element there
            let idx = self.missing.pop().unwrap();
            let (old_val, old_version) = self.vec.get_mut(idx as usize).unwrap();
            *old_val = Some(elem);
            *old_version += 1;
            idx
        }
    }
    /// Get the index of the next element that we will add
    pub fn get_next_idx(&self) -> u64 {
        // Normal push
        if self.missing.is_empty() {
            return  to_id(IndexPair::new(self.vec.len(), 0));
        }
        // Shove
        *self.missing.last().unwrap()
    }
    /// Remove an element that was already added
    pub fn remove(&mut self, id: u64) -> Option<T> {
        let pair = from_id(id);
        self.missing.push(id);
        let (elem, version) = self.vec.get_mut(pair.index as usize)?;
        std::mem::take(elem)
    }
    /// Get a reference to an element in the ordered vector
    pub fn get(&self, idx: usize) -> Option<&T> {
        self.vec.get(idx)?.0.as_ref()
    }
    /// Get a mutable reference to an element in the ordered vector
    pub fn get_mut(&mut self, idx: usize) -> Option<&mut T> {
        self.vec.get_mut(idx)?.0.as_mut()
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

        std::mem::replace(&mut self.vec, empty)
    }
}

/// Iter magic
impl<T> OrderedVec<T> {
    /// Convert this into an iterator
    pub fn into_iter(self) -> impl Iterator<Item = T> {
        self.vec.into_iter().flatten()
    }
    /// Get an iterator over the valid elements
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.vec.iter().filter_map(|val| val.as_ref())
    }
    /// Get an iterator over the valid elements
    pub fn iter_indexed(&self) -> impl Iterator<Item = (usize, &T)> {
        self.vec
            .iter()
            .enumerate()
            .filter_map(|(index, val)| val.as_ref().map(|val| (index, val)))
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
            .filter_map(|(index, val)| val.as_mut().map(|val| (index, val)))
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
