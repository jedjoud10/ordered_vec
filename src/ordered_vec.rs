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
    pub(crate) missing: Vec<usize>,
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
            to_id(IndexPair::new(self.vec.len() - 1, 0))
        } else {
            // If we have some null elements, we can validate the given element there
            let index = self.missing.pop().unwrap();
            let (old_val, old_version) = self.vec.get_mut(index as usize).unwrap();
            *old_val = Some(elem);
            *old_version += 1;
            // Create an ID from an index and old version
            to_id(IndexPair::new(index, *old_version))
        }
    }
    /// Get the index of the next element that we will add
    pub fn get_next_index(&self) -> usize {
        // Normal push
        if self.missing.is_empty() {
            return self.vec.len();
        }
        // Shove
        *self.missing.last().unwrap()
    }
    /// Get the ID of the next element that we will add
    pub fn get_next_id(&self) -> u64 {
        // Normal push
        if self.missing.is_empty() {
            return to_id(IndexPair::new(self.vec.len(), 0));
        }
        // Shove
        let index = *self.missing.last().unwrap();
        let (_, version) = self.vec.get(index).unwrap();
        to_id(IndexPair::new(index, *version + 1))
    }
    /// Remove an element that is contained in the vec
    pub fn remove(&mut self, id: u64) -> Option<T> {
        let pair = from_id(id);
        self.missing.push(pair.index as usize);
        let (elem, version) = self.vec.get_mut(pair.index as usize)?;
        // Only remove if the version is the same as well
        if pair.version != *version {
            return None;
        }
        std::mem::take(elem)
    }
    /// Remove an element that is contained in the vec. This does not check if the element's version matches up with the ID!
    pub fn remove_index(&mut self, index: usize) -> Option<T> {
        self.missing.push(index);
        let (elem, _) = self.vec.get_mut(index as usize)?;
        std::mem::take(elem)
    }
    /// Get a reference to an element in the ordered vector
    pub fn get(&self, id: u64) -> Option<&T> {
        let pair = from_id(id);
        // First of all check if we *might* contain the cell
        return if (pair.index as usize) < self.vec.len() {
            // We contain the cell, but it might be null
            let (cell, version) = self.vec.get(pair.index as usize)?;
            // Check if the versions are the same
            if pair.version == *version { cell.as_ref() } else { None }
        } else {
            // We do not contain the cell at all
            None
        };
    }
    /// Get a mutable reference to an element in the ordered vector
    pub fn get_mut(&mut self, id: u64) -> Option<&mut T> {
        let pair = from_id(id);
        // First of all check if we *might* contain the cell
        return if (pair.index as usize) < self.vec.len() {
            // We contain the cell, but it might be null
            let (cell, version) = self.vec.get_mut(pair.index as usize)?;
            // Check if the versions are the same
            if pair.version == *version { cell.as_mut() } else { None }
        } else {
            // We do not contain the cell at all
            None
        };
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
        rep.into_iter().map(|(val, _)| val).collect::<Vec<_>>() 
    }
}

/// Iter magic
impl<T> OrderedVec<T> {
    /// Convert this into an iterator
    pub fn into_iter(self) -> impl Iterator<Item = (u64, T)> {
        self.vec.into_iter().enumerate().filter_map(|(index, (val, version))| { 
            val.map(|val| (to_id(IndexPair::new(index, version)), val))
        })
    }
    /// Get an iterator over the valid elements
    pub fn iter_elements(&self) -> impl Iterator<Item = &T> {
        self.vec.iter().filter_map(|(val, _)| val.as_ref())
    }
    /// Get a mutable iterator over the valid elements
    pub fn iter_elements_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.vec.iter_mut().filter_map(|(val, _)| val.as_mut())
    }
    /// Get an iterator over the valid elements, but with the ID of each element
    pub fn iter(&self) -> impl Iterator<Item = (u64, &T)> {
        self.vec.iter().enumerate().filter_map(|(index, (val, version))| { 
            val.as_ref().map(|val| (to_id(IndexPair::new(index, *version)), val))
        })
    }
    /// Get a mutable iterator over the valid elements, but with the ID of each element
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (u64, &mut T)> {
        self.vec.iter_mut().enumerate().filter_map(|(index, (val, version))| { 
            val.as_mut().map(|val| (to_id(IndexPair::new(index, *version)), val))
        })
    }
    /// Get an iterator over the indices of the null elements
    pub fn iter_invalid(&self) -> impl Iterator<Item = &usize> {
        self.missing.iter()
    }
    /// Drain the elements that only return true. This will return just an Iterator of the index and value of the drained elements
    pub fn my_drain<F>(&mut self, mut filter: F) -> impl Iterator<Item = (u64, T)> + '_
    where
        F: FnMut(u64, &T) -> bool,
    {
        // Keep track of the IDs that we must remove
        let mut removed_ids: Vec<u64> = Vec::new();
        for (index, (val, version)) in self.vec.iter_mut().enumerate() {
            if let Some(val) = val {
                // If it validates the filter, we must remove it
                let id = to_id(IndexPair::new(index, *version));
                if filter(id, val) {
                    // We must remove this value
                    removed_ids.push(id);
                }
            } 
        }
        // Now we can actually remove the objects
        removed_ids.into_iter().map(|id| (id, self.remove(id).unwrap()))
    }
}

/// Traits
impl<T> Index<u64> for OrderedVec<T> {
    type Output = T;
    fn index(&self, index: u64) -> &Self::Output {
        self.get(index).unwrap()
    }
}

impl<T> IndexMut<u64> for OrderedVec<T> {
    fn index_mut(&mut self, index: u64) -> &mut Self::Output {
        self.get_mut(index).unwrap()
    }
}
