use std::{
    fmt::Debug,
    ops::{Index, IndexMut},
    sync::atomic::{AtomicUsize, Ordering::Relaxed}, collections::{BTreeSet, HashSet},
};

use crate::utils::{from_id, to_id, IndexPair};
/// A collection that keeps the ordering of its elements, even when deleting an element
/// However, this collection can be shared between threads
/// We can *guess* what the index is for an element that we must add
/// We can use **get**, and **get_next_idx_increment** on other threads, but that is all
/// We must do the rest of our operations using an external messaging system
pub struct ShareableOrderedVec<T> {
    /// A list of the current elements in the list
    pub(crate) vec: Vec<(Option<T>, Option<u32>)>,
    /// A list of the indices that contain a null element, so whenever we add a new element, we will add it there
    pub(crate) missing: Vec<usize>,
    /// A counter that increases every time we add an element to the list in other threads, before the main update
    pub(crate) counter: AtomicUsize,
    /// The current length of the vector. This will increase when we add an elements that is outisde of the current vector
    pub(crate) length: AtomicUsize,
}

impl<T> Default for ShareableOrderedVec<T> {
    fn default() -> Self {
        Self {
            vec: Vec::new(),
            missing: Vec::new(),
            counter: AtomicUsize::new(0),
            length: AtomicUsize::new(0),
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
            .field("missing", &self.missing)
            .finish()
    }
}

impl<T> ShareableOrderedVec<T> {
    /// Add an element to the ordered vector, but at a specific index (we get that through the ID)
    /// This will return the last element that was at that index, if possible
    pub fn insert(&mut self, id: u64, elem: T) -> Option<T> {
        // Check the length first
        let pair = from_id(id);
        let idx = pair.index as usize;
        if idx >= self.vec.len() {
            // We must resize and add
            self.vec.resize_with(idx, || {
                // We want to fill the gap with just empty values
                (None, None)
            });
            // Actually insert the elements
            self.vec.push((Some(elem), Some(pair.version)));
            if self.vec.len() - 1 != idx {
                panic!()
            }
            let old_ctr = self.counter.swap(0, Relaxed);
            if old_ctr != 0 {
                // Since we have read using the atomic counter, we can just remove the missing indices before it
                // The counter might be greater than the amount of missing cells
                if old_ctr >= self.missing.len() {
                    self.missing.clear()
                } else {
                    self.missing.drain(0..old_ctr);
                }
            }
            self.length.store(self.vec.len(), Relaxed);
            None
        } else {
            // Simple overwrite
            // Replace
            let (old_val, old_version) = self.vec.get_mut(idx).unwrap();
            // If the value was uninitialized, we must initialize it
            if old_version.is_none() {
                *old_version = Some(0);

                std::mem::replace(old_val, Some(elem))
            } else {
                *old_version.as_mut().unwrap() += 1;

                std::mem::replace(old_val, Some(elem))
            }
        }
    }
    /// Get the ID of the next element that we will add. If we call this twice, without inserting any elements, it will not change
    pub fn get_next_id(&self) -> u64 {
        // Normal push
        let index = self.missing.last().cloned().unwrap_or(self.vec.len());
        let (_, version) = self.vec.get(index).unwrap();
        to_id(IndexPair::new(index, version.unwrap_or(0)))
    }
    /// Check the next index where we can add an element, but also increment the counter, so it won't be the same index
    /// This assumes that we wille eventually insert an element at said index
    pub fn get_next_id_increment(&self) -> u64 {
        // Try to get an empty cell, if we couldn't just use the length as the index
        let ctr = self.counter.fetch_add(1, Relaxed);
        let index = self
            .missing
            .get(ctr)
            .cloned()
            .unwrap_or_else(|| self.length.fetch_add(1, Relaxed));
        let version = if let Some((_, index)) = self.vec.get(index) { index.unwrap_or(0) + 1 } else { 0 };
        to_id(IndexPair::new(index, version))
    }
    /// Remove an element that is contained in the shareable vec
    pub fn remove(&mut self, id: u64) -> Option<T> {
        let pair = from_id(id);
        self.missing.push(pair.index as usize);
        let (elem, version) = self.vec.get_mut(pair.index as usize)?;
        // Only remove if the version is the same as well
        if pair.version != *(version.as_ref()?) {
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
            if pair.version == *(version.as_ref()?) {
                cell.as_ref()
            } else {
                None
            }
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
            if pair.version == *(version.as_ref()?) {
                cell.as_mut()
            } else {
                None
            }
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
    /// Clear the whole shareable ordered vector
    pub fn clear(&mut self) -> Vec<Option<T>> {
        // Simple clear
        let rep = std::mem::take(&mut self.vec);
        self.missing.clear();
        rep.into_iter().map(|(val, _)| val).collect::<Vec<_>>()
    }
}

/// Iter magic
impl<T> ShareableOrderedVec<T> {
    /// Convert this into an iterator
    pub fn into_iter(self) -> impl Iterator<Item = (u64, T)> {
        self.vec
            .into_iter()
            .enumerate()
            .filter_map(|(index, (val, version))| {
                val.map(|val| (to_id(IndexPair::new(index, version.unwrap())), val))
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
        self.vec
            .iter()
            .enumerate()
            .filter_map(|(index, (val, version))| {
                val.as_ref()
                    .map(|val| (to_id(IndexPair::new(index, *(version.as_ref().unwrap()))), val))
            })
    }
    /// Get a mutable iterator over the valid elements, but with the ID of each element
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (u64, &mut T)> {
        self.vec
            .iter_mut()
            .enumerate()
            .filter_map(|(index, (val, version))| {
                val.as_mut()
                    .map(|val| (to_id(IndexPair::new(index, *(version.as_ref().unwrap()))), val))
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
                let id = to_id(IndexPair::new(index, *(version.as_ref().unwrap())));
                if filter(id, val) {
                    // We must remove this value
                    removed_ids.push(id);
                }
            }
        }
        // Now we can actually remove the objects
        removed_ids
            .into_iter()
            .map(|id| (id, self.remove(id).unwrap()))
    }
}

/// Traits
impl<T> Index<u64> for ShareableOrderedVec<T> {
    type Output = T;
    fn index(&self, index: u64) -> &Self::Output {
        self.get(index).unwrap()
    }
}

impl<T> IndexMut<u64> for ShareableOrderedVec<T> {
    fn index_mut(&mut self, index: u64) -> &mut Self::Output {
        self.get_mut(index).unwrap()
    }
}
