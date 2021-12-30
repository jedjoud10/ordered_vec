use std::ops::{Index, IndexMut};


// A collection that keeps the ordering of it's elements, even when deleting an element
pub struct OrderedVec<T> {
    vec: Vec<Option<T>>, // A list of the current elements in the list
    missing: Vec<usize>, // A list of the indices that contain a null element, so whenever we add a new element, we will add it there
}

impl<T> Default for OrderedVec<T> {
    fn default() -> Self {
        Self { vec: Vec::new(), missing: Vec::new() }
    }
}

// Actual code
impl<T> OrderedVec<T> {
    // Add an element to the ordered vector
    pub fn push_shove(&mut self, elem: T) -> usize {
        if self.missing.is_empty() { 
            // Add the element normally
            self.vec.push(Some(elem)); return self.vec.len() - 1;
        } 
        else {
            // If we have some null elements, we can validate the given element there
            let idx = self.missing.pop().unwrap();
            *self.vec.get_mut(idx).unwrap() = Some(elem); 
            return idx;
        }
    }
    // Get the index of the next element that we will add
    pub fn get_next_idx(&self) -> usize {
        // Normal push
        if self.missing.is_empty() { return self.vec.len(); }
        // Shove
        *self.missing.last().unwrap()
    }
    // Remove an element that was already added
    pub fn remove(&mut self, idx: usize) -> Option<T> {
        self.missing.push(idx);
        let elem = self.vec.get_mut(idx)?;
        let elem = std::mem::take(elem);
        elem
    }
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
}

// Iter magic
impl<T> OrderedVec<T> {
    // Get an iterator over the valid elements
    pub fn iter(&self) -> impl Iterator<Item = &T>{
        self.vec.iter().filter_map(|x| x.as_ref())
    }
    // Get a mutable iterator over the valid elements
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.vec.iter_mut().filter_map(|x| x.as_mut())
    }
    // Get an iterator over the indices of the null elements
    pub fn iter_invalid(&self) -> impl Iterator<Item = &usize>{
        self.missing.iter()
    }
}

// Traits
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