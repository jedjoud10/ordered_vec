use std::{
    cell::RefCell,
    marker::PhantomData,
    sync::{
        atomic::{AtomicUsize, Ordering::Relaxed},
        Arc, RwLock,
    },
};

/// A shareable state that can be created by a ShareableOrderedVec
/// This helps since we cannot get, get_mut, remove or push_shove on other threads, so it makes it a bit safer
#[derive(Clone)]
pub struct ShareableOrderedVecState<T> {
    /// A list of the indices that contain a null element, so whenever we add a new element, we will add it there
    pub(crate) missing: Arc<RwLock<Vec<usize>>>,
    /// A counter that increases every time we add an element to the list in other threads, before the main update
    pub(crate) counter: Arc<AtomicUsize>,
    /// The current length of the vector. This will increase when we add an elements that is outisde of the current vector
    pub(crate) length: Arc<AtomicUsize>,
    /// Phantom data
    pub(crate) _phantom: PhantomData<*const T>,
}

unsafe impl<T> Sync for ShareableOrderedVecState<T> {}
unsafe impl<T> Send for ShareableOrderedVecState<T> {}

impl<T> ShareableOrderedVecState<T> {
    /// Check the next ID where we can add an element, but also increment the counter, so it won't be the same ID
    pub fn get_next_id_increment(&self) -> usize {
        // Try to get an empty cell, if we couldn't just use the length as the index
        let missing = self.missing.as_ref().read().unwrap();
        let ctr = self.counter.fetch_add(1, Relaxed);
        missing
            .get(ctr)
            .cloned()
            .unwrap_or_else(|| self.length.fetch_add(1, Relaxed))
    }
}
