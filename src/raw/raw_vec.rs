use std::{marker::PhantomData, alloc::Layout, ptr::NonNull};

// A raw vector that can grow it's allocated size
pub(crate) struct RawVec {   
    
}

impl RawVec {
    

    // Create a new empty raw vector, and set our local layout of type T
    pub unsafe fn new<T: Sized>() -> Self {
        Self {
            ptr: NonNull::dangling(),
            cap: 0,
            _marker: Default::default(),
            type_layout: Layout::new::<T>(),
        }
    }
}