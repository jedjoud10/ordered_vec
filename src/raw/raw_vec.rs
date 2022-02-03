use std::{marker::PhantomData, alloc::Layout, ptr::NonNull};

// A raw vector that can grow it's allocated size
pub(crate) struct RawVec {   
    pub(crate) ptr: NonNull<u8>,
    pub(crate) cap: usize,
    _marker: PhantomData<u8>,

    // The layout for the type that we must represent
    pub(crate) type_layout: Layout,
}

impl RawVec {
    // Grow the raw vector so it can be able twice as much elements before allocating
    unsafe fn grow(&mut self) {
        // Get the new cap and layout (the new cap is in bytes)
        let (new_cap, new_layout) = if self.cap == 0 {
            (1 * self.type_layout.size(), self.type_layout)
        } else {
            // The grow policy is to multiply the currently allocated space by 2
            let new_cap = self.cap * 2 * self.type_layout.size();
            let new_layout = Layout::from_size_align_unchecked(new_cap, self.type_layout.align());
            (new_cap, new_layout)
        };

        // Ensure that the new allocation doesn't exceed `isize::MAX` bytes.
        assert!(new_layout.size() <= isize::MAX as usize, "Allocation too large");

        let new_ptr = if self.cap == 0 {
            std::alloc::alloc(new_layout)
        } else {
            let old_layout = Layout::from_size_align_unchecked(self.cap, self.type_layout.align());
            let old_ptr = self.ptr.as_ptr() as *mut u8;
            std::alloc::realloc(old_ptr, old_layout, new_layout.size())
        };

        // If allocation fails, `new_ptr` will be null, in which case we abort.
        self.ptr = match NonNull::new(new_ptr as *mut u8) {
            Some(p) => p,
            None => std::alloc::handle_alloc_error(new_layout),
        };
        self.cap = new_cap;
    }

    // Create a new empty raw vector, and set our local layout of type T
    pub unsafe fn new<T: Sized>() -> Self {
        Self {
            ptr: NonNull::dangling(),
            cap: todo!(),
            _marker: Default::default(),
            type_layout: todo!(),
        }
    }
}