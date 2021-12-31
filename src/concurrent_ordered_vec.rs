use std::{ops::{Index, IndexMut}, fmt::Debug, sync::{atomic::{AtomicUsize, AtomicU64, Ordering::{Relaxed, self}}, RwLock, mpsc::{Sender, Receiver}, Arc, Mutex, MutexGuard}, cell::{RefCell, Ref}};

// A simple command
enum ConcurrentOrderedVecCommand<T> {
    Add(T),
    OverWrite(T, usize),
    Remove(usize),
}

// Ordered vec, but this can be acessed from multiple threads. 
// We can only have one thread that actually updates it's state however
pub struct ConcurrentOrderedVec<T> {
    vec: Arc<RwLock<Vec<Option<T>>>>, // A list of the current elements in the list
    missing: Arc<RwLock<Vec<usize>>>, // A list of the indices that contain a null element, so whenever we add a new element, we will add it there
    len: Arc<AtomicUsize>,
    cmd_counter: Arc<AtomicU64>,
    tx: Sender<(ConcurrentOrderedVecCommand<T>, u64)>,
    rx: Option<Arc<Receiver<(ConcurrentOrderedVecCommand<T>, u64)>>>,
    thread_id: std::thread::ThreadId,
}

impl<T> Clone for ConcurrentOrderedVec<T> {
    fn clone(&self) -> Self {
        Self { 
            vec: self.vec.clone(),
            missing: self.missing.clone(),
            len: self.len.clone(),
            cmd_counter: self.cmd_counter.clone(),
            tx: self.tx.clone(),
            rx: self.rx.clone(),
            thread_id: self.thread_id.clone()
        }
    }
}

unsafe impl<T> Sync for ConcurrentOrderedVec<T> {}
unsafe impl<T> Send for ConcurrentOrderedVec<T> {}

impl<T> Default for ConcurrentOrderedVec<T> {
    fn default() -> Self {
        let (tx, rx) = std::sync::mpsc::channel::<(ConcurrentOrderedVecCommand<T>, u64)>();
        Self { 
            vec: Default::default(),
            missing: Default::default(),
            len: Default::default(),
            cmd_counter: Default::default(),
            tx,
            rx: Some(Arc::new(rx)),
            thread_id: std::thread::current().id()
        }
    }
}

impl<T> Debug for ConcurrentOrderedVec<T> where T: Debug {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConcurrentOrderedVec").field("vec", &self.vec).field("missing", &self.missing).finish()
    }
}

// Actual code
impl<T> ConcurrentOrderedVec<T> {
    // Add an element to the ordered vector
    pub fn push_shove(&mut self, elem: T) -> usize {
        if self.thread_id != std::thread::current().id() {
            // Send the command
            let cmd = self.cmd_counter.fetch_add(1, Ordering::Relaxed);
            if self.missing.read().unwrap().is_empty() { 
                // Add the element normally
                self.tx.send((ConcurrentOrderedVecCommand::Add(elem), cmd)).unwrap();
                self.len.fetch_add(1, Ordering::Relaxed)
            } 
            else {
                // If we have some null elements, we can validate the given element there
                let mut missing = self.missing.write().unwrap();
                let idx = missing.pop().unwrap();
                self.tx.send((ConcurrentOrderedVecCommand::OverWrite(elem, idx), cmd)).unwrap();
                return idx;
            }
        } else {
            // Only do this if we are not on the creation thread
            if self.missing.read().unwrap().is_empty() { 
                // Add the element normally
                let mut writable = self.vec.write().unwrap(); 
                writable.push(Some(elem)); return writable.len() - 1;
            } 
            else {
                // If we have some null elements, we can validate the given element there
                let mut writable = self.missing.write().unwrap();
                let idx = writable.pop().unwrap();
                let mut overwrite_vec = self.vec.write().unwrap();
                *overwrite_vec.get_mut(idx).unwrap() = Some(elem); 
                return idx;
            }
        }
    }
    // Get the index of the next element that we will add
    pub fn get_next_idx(&self) -> usize {
        if self.thread_id != std::thread::current().id() {
            // Read from the atomic if needed
            let readable = self.missing.read().unwrap();
            if readable.is_empty() { return self.len.load(Ordering::Relaxed); }
            *readable.last().unwrap()
        } else {
            // Normal push
            let readable = self.missing.read().unwrap();
            if readable.is_empty() { return self.vec.read().unwrap().len(); }
            // Shove
            *self.missing.read().unwrap().last().unwrap()
        }
    }
    // Remove an element that was already added
    pub fn remove(&mut self, idx: usize) -> Option<T> {
        let cmd = self.cmd_counter.fetch_add(1, Ordering::Relaxed);
        if self.thread_id != std::thread::current().id() {  
            // Send the command
            let mut writable = self.missing.write().unwrap(); 
            writable.push(idx);
            self.tx.send((ConcurrentOrderedVecCommand::Remove(idx), cmd)).unwrap();
            None
        } else {
            // Do it normally
            self.missing.write().unwrap().push(idx);
            let mut writable = self.vec.write().unwrap(); 
            let elem = writable.get_mut(idx)?;
            let elem = std::mem::take(elem);
            elem
        }
    }
    // Update
    pub fn update(&mut self) {
        let mut x = self.rx.as_ref().unwrap().try_iter().collect::<Vec<_>>();
        x.sort_by(|(_, a), (_, b)| a.cmp(b));
        //let vec = self.vec
        let mut vec = self.vec.as_ref().write().unwrap();
        for (command, _) in x {
            match command {
                ConcurrentOrderedVecCommand::Add(val) => {
                    // Add the element
                    vec.push(Some(val));
                },
                ConcurrentOrderedVecCommand::OverWrite(val, idx) => {
                    // Overwrite the element
                    let current_val = vec.get_mut(idx).unwrap();
                    let old_val = std::mem::replace(current_val, Some(val));
                },
                ConcurrentOrderedVecCommand::Remove(idx) => {                    
                    let elem = vec.get_mut(idx).unwrap();
                    let elem = std::mem::take(elem);
                },
            }
        }
    }
}
/*
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
*/