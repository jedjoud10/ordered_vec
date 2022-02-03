use crate::utils::{from_id, to_id, IndexPair};
use std::mem::size_of;
use std::{alloc::Layout, marker::PhantomData, ptr::NonNull};

/// A raw ordered vector that stores it's elements without the need of a generic, and checks for type layout equality at runtime
/// Totally not stolen from here https://doc.rust-lang.org/nomicon/vec/vec.html
pub struct RawOrderedVec {
    /// A list of the indices that contain a null element, so whenever we add a new element, we will add it there
    missing: Vec<usize>,
    /// How many elements we have (doesn't matter if they uninitialized or not)
    len: usize,
    /// How many more elements we can store until we reallocate
    cap: usize,
    /// The underlying pointer for out data
    ptr: NonNull<u8>,
    /// The layout for the type that we must represent (T, u32)
    type_layout: Layout,

    _marker: PhantomData<u8>,
}

impl RawOrderedVec {
    // Grow the raw vector so it can be able twice as much elements before allocating
    unsafe fn grow(&mut self) {
        // Get the new cap and layout (the new cap is in bytes)
        let (new_cap, new_layout) = if self.cap == 0 {
            (1, self.type_layout)
        } else {
            // The grow policy is to multiply the currently allocated space by 2
            let new_cap = self.cap * 2;
            let new_layout = Layout::from_size_align_unchecked(
                new_cap * self.type_layout.size(),
                self.type_layout.align(),
            );
            (new_cap, new_layout)
        };

        // Ensure that the new allocation doesn't exceed `isize::MAX` bytes.
        assert!(
            new_layout.size() <= isize::MAX as usize,
            "Allocation too large"
        );
        let new_ptr = if self.cap == 0 {
            std::alloc::alloc(new_layout)
        } else {
            let old_layout = Layout::from_size_align_unchecked(
                self.cap * self.type_layout.size(),
                self.type_layout.align(),
            );
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
    /// Check for type layout equality
    fn valid_layout<T: Sized>(&self) -> bool {
        Layout::new::<(Option<T>, u32)>() == self.type_layout
    }
    /// Get unchecked, unsafe
    unsafe fn get_unchecked_raw<T: Sized>(&self, index: usize) -> &(Option<T>, u32) {
        let val =
            std::slice::from_raw_parts(self.ptr.as_ptr() as *const (Option<T>, u32), self.len);
        &val[index]
    }
    /// Get mut unchecked, unsafe
    unsafe fn get_unchecked_mut_raw<T: Sized>(&mut self, index: usize) -> &mut (Option<T>, u32) {
        let val =
            std::slice::from_raw_parts_mut(self.ptr.as_ptr() as *mut (Option<T>, u32), self.len);
        &mut val[index]
    }
    /// Get the version for a specific index
    unsafe fn get_version_raw(&self, index: usize) -> &u32 {
        let val =
            std::slice::from_raw_parts(self.ptr.as_ptr().sub(size_of::<u32>()) as *mut u32, 1);
        &val[index]
    }
    /// Create a new raw ordered vector with a specific type
    pub unsafe fn new<T: Sized>() -> Self {
        Self {
            ptr: NonNull::dangling(),
            len: 0,
            cap: 0,
            missing: Vec::new(),
            _marker: Default::default(),
            type_layout: Layout::new::<(Option<T>, u32)>(),
        }
    }
    /// Length of all the elements
    pub fn len(&self) -> usize {
        self.len
    }
    /// Internal capacity
    pub fn cap(&self) -> usize {
        self.cap
    }

    /// Add an element to the ordered vector
    pub unsafe fn push_shove<T: Sized>(&mut self, elem: T) -> u64 {
        assert!(
            self.valid_layout::<T>(),
            "Generic type does not match internal type layout!"
        );
        // Check for type layout equality
        if self.missing.is_empty() {
            // Add the element normally

            // Check if we have enough allocated space to be able to push this element
            if self.cap() == self.len {
                // We must allocate
                self.grow();
            }
            // Always write
            std::ptr::write(self.ptr.as_ptr().add(self.len) as *mut (T, u32), (elem, 0));
            self.len += 1;
            to_id(IndexPair::new(self.len - 1, 0))
        } else {
            // If we have some null elements, we can validate the given element there
            let index = self.missing.pop().unwrap();
            let (old_val, old_version) = self.get_unchecked_mut_raw::<T>(index);
            *old_val = Some(elem);
            *old_version += 1;
            // Create an ID from an index and old version
            to_id(IndexPair::new(index, *old_version))
        }
    }
    /// Get the index of the next element that we will add
    pub unsafe fn get_next_index(&self) -> usize {
        // Normal push
        if self.missing.is_empty() {
            return self.len;
        }
        // Shove
        *self.missing.last().unwrap()
    }
    /// Get the ID of the next element that we will add
    pub unsafe fn get_next_id(&self) -> u64 {
        // Normal push
        if self.missing.is_empty() {
            return to_id(IndexPair::new(self.len, 0));
        }
        // Shove
        let index = *self.missing.last().unwrap();
        let version = self.get_version_raw(index);
        to_id(IndexPair::new(index, *version + 1))
    }
    /// Remove an element that is contained in the vec
    pub unsafe fn remove<T>(&mut self, id: u64) -> Option<T> {
        assert!(
            self.valid_layout::<T>(),
            "Generic type does not match internal type layout!"
        );
        let pair = from_id(id);
        self.missing.push(pair.index as usize);
        let (elem, version) = self.get_unchecked_mut_raw::<T>(pair.index as usize);
        // Only remove if the version is the same as well
        if pair.version != *version {
            return None;
        }
        std::mem::take(elem)
    }
    /// Remove an element that is contained in the vec. This does not check if the element's version matches up with the ID!
    pub unsafe fn remove_index<T>(&mut self, index: usize) -> Option<T> {
        assert!(
            self.valid_layout::<T>(),
            "Generic type does not match internal type layout!"
        );
        self.missing.push(index);
        let (elem, _) = self.get_unchecked_mut_raw(index as usize);
        std::mem::take(elem)
    }
    /// Get a reference to an element in the ordered vector
    pub unsafe fn get<T>(&self, id: u64) -> Option<&T> {
        assert!(
            self.valid_layout::<T>(),
            "Generic type does not match internal type layout!"
        );
        let pair = from_id(id);
        // First of all check if we *might* contain the cell
        return if (pair.index as usize) < self.len {
            // We contain the cell, but it might be null
            let (cell, version) = self.get_unchecked_raw::<T>(pair.index as usize);
            // Check if the versions are the same
            if pair.version == *version {
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
    pub unsafe fn get_mut<T>(&mut self, id: u64) -> Option<&mut T> {
        assert!(
            self.valid_layout::<T>(),
            "Generic type does not match internal type layout!"
        );
        let pair = from_id(id);
        // First of all check if we *might* contain the cell
        return if (pair.index as usize) < self.len {
            // We contain the cell, but it might be null
            let (cell, version) = self.get_unchecked_mut_raw::<T>(pair.index as usize);
            // Check if the versions are the same
            if pair.version == *version {
                cell.as_mut()
            } else {
                None
            }
        } else {
            // We do not contain the cell at all
            None
        };
    }
    /// Pop the last element from the ordered vec
    pub unsafe fn pop(&mut self) -> Option<()> {
        if self.len == 0 {
            None
        } else {
            self.len -= 1;
            Some(())
        }
    }
    /// Get the number of valid elements in the ordered vector
    pub fn count(&self) -> usize {
        self.len - self.missing.len()
    }
    /// Get the number of invalid elements in the ordered vector
    pub fn count_invalid(&self) -> usize {
        self.missing.len()
    }
}

impl Drop for RawOrderedVec {
    fn drop(&mut self) {
        // Don't leak memory
        unsafe {
            if self.cap() != 0 {
                while self.pop().is_some() {}
                let layout = Layout::from_size_align(
                    self.type_layout.size() + size_of::<u32>(),
                    self.type_layout.align(),
                )
                .unwrap();
                std::alloc::dealloc(self.ptr.as_ptr(), layout);
            }
        }
    }
}
