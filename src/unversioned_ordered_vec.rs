use std::{
    fmt::Debug,
    ops::{Index, IndexMut},
};

use crate::utils::{from_id, to_id, IndexPair};

/// A collection that keeps the ordering of its elements, even when deleting an element
pub struct UnversionnedOrderedVec<T> {
    /// A list of the current elements in the list
    pub(crate) vec: Vec<Option<T>>,
    /// A list of the indices that contain a null element, so whenever we add a new element, we will add it there
    pub(crate) missing: Vec<usize>,
}

impl<T> Clone for UnversionnedOrderedVec<T>
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

impl<T> Debug for UnversionnedOrderedVec<T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnversionnedOrderedVec")
            .field("vec", &self.vec)
            .field("missing", &self.missing)
            .finish()
    }
}

impl<T> Default for UnversionnedOrderedVec<T> {
    fn default() -> Self {
        Self {
            vec: Vec::new(),
            missing: Vec::new(),
        }
    }
}

/// Actual code
impl<T> UnversionnedOrderedVec<T> {
    /// New
    pub fn new() -> Self {
        Self::default()
    }
    /// Create Self using already existing elements
    pub fn from_valids(vals: Vec<T>) -> Self {
        Self {
            vec: vals
                .into_iter()
                .map(|x| Some(x))
                .collect::<Vec<Option<T>>>(),
            missing: Vec::new(),
        }
    }
    /// Add an element to the ordered vector
    pub fn push_shove(&mut self, elem: T) -> usize {
        if self.missing.is_empty() {
            // Add the element normally
            self.vec.push(Some(elem));
            self.vec.len() - 1
        } else {
            // If we have some null elements, we can validate the given element there
            let index = self.missing.pop().unwrap();
            let old_val = self.vec.get_mut(index as usize).unwrap();
            *old_val = Some(elem);
            index
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
    /// Remove an element that is contained in the vec
    pub fn remove(&mut self, index: usize) -> Option<T> {
        self.missing.push(index);
        let elem = self.vec.get_mut(index)?;
        std::mem::take(elem)
    }
    /// Get a reference to an element in the ordered vector
    pub fn get(&self, index: usize) -> Option<&T> {
        self.vec.get(index)?.as_ref()
    }
    /// Get a mutable reference to an element in the ordered vector
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.vec.get_mut(index)?.as_mut()
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
        // Simple clear
        let rep = std::mem::take(&mut self.vec);
        self.missing.clear();
        rep.into_iter().collect::<Vec<_>>()
    }
}

/// Iter magic
impl<T> UnversionnedOrderedVec<T> {
    /// Convert this into an iterator
    pub fn into_iter(self) -> impl Iterator<Item = (usize, T)> {
        self.vec
            .into_iter()
            .enumerate()
            .filter_map(|(index, val)| val.map(|val| (index, val)))
    }
    /// Get an iterator over the valid elements
    pub fn iter_elements(&self) -> impl Iterator<Item = &T> {
        self.vec.iter().filter_map(|val| val.as_ref())
    }
    /// Get a mutable iterator over the valid elements
    pub fn iter_elements_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.vec.iter_mut().filter_map(|val| val.as_mut())
    }
    /// Get an iterator over the valid elements, but with the ID of each element
    pub fn iter(&self) -> impl Iterator<Item = (usize, &T)> {
        self.vec
            .iter()
            .enumerate()
            .filter_map(|(index, val)| val.as_ref().map(|val| (index, val)))
    }
    /// Get a mutable iterator over the valid elements, but with the ID of each element
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (usize, &mut T)> {
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
        // Keep track of the indices that we must remove
        let mut removed_indices: Vec<usize> = Vec::new();
        for (index, val) in self.vec.iter_mut().enumerate() {
            if let Some(val) = val {
                // If it validates the filter, we must remove it
                if filter(index, val) {
                    // We must remove this value
                    removed_indices.push(index);
                }
            }
        }
        // Now we can actually remove the objects
        removed_indices
            .into_iter()
            .map(|id| (id, self.remove(id).unwrap()))
    }
}

/// Traits
impl<T> Index<usize> for UnversionnedOrderedVec<T> {
    type Output = T;
    fn index(&self, index: usize) -> &Self::Output {
        self.get(index).unwrap()
    }
}

impl<T> IndexMut<usize> for UnversionnedOrderedVec<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.get_mut(index).unwrap()
    }
}
