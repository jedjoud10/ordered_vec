use std::{ops::{Index, IndexMut}, fmt::Debug, sync::{atomic::{AtomicU64, Ordering}, Mutex, mpsc::{Sender, Receiver}, RwLock}, thread::ThreadId};


// A channel message that will be sent to the creation thread of the concurrent ordered vec
enum Command<T> {
    Add(T),
    OverWrite(Option<T>, usize),
}

// The concurrent version of OrderedVec<T>
// PS: This is not fully concurrent, since we cannot retrieve shared mutable data
pub struct HalfConcurrentOrderedVec<T>
    where T: Sync {
    // We first store all of our bool values in a batch of Atomicu64s
    // The boolean value tells us if the atomic is completely filled or not
    batches: RwLock<Vec<AtomicU64>>,
    
    // Same stuff here
    vec: RwLock<Vec<Option<T>>>, // A list of the current elements in the list

    // The channels that will be used to send the messages to the creation thread
    tx: Sender<(u64, Command<T>)>,
    rx: Option<Receiver<(u64, Command<T>)>>,
    creation_thread_id: ThreadId, // ID of the creation thread
    ctr: AtomicU64, // An atomic for counting the number of commands sent to the creation thread. We must use this so we can order the commands when we receive them
}

unsafe impl<T> Sync for HalfConcurrentOrderedVec<T> where T: Sync{}

impl<T> Default for HalfConcurrentOrderedVec<T>
    where T: Sync {
    fn default() -> Self {
        let creation_thread_id = std::thread::current().id();
        let (tx, rx) = std::sync::mpsc::channel::<(u64, Command<T>)>();
        // The channel use to communicate with the creation thread (the current one)
        Self {
            batches: RwLock::new(Vec::new()),
            vec: RwLock::new(Vec::new()),
            tx,
            rx: Some(rx),
            creation_thread_id: creation_thread_id,
            ctr: AtomicU64::new(0),
        }
    }
}

// Actual code
impl<T> HalfConcurrentOrderedVec<T> 
    where T: Sync{
    // Send a command to the creation thread
    fn send_command(&self, command: Command<T>) {
        let ctr = self.ctr.fetch_add(1, Ordering::Relaxed);
        self.tx.send((ctr, command)).unwrap();
    }
    // Get the next index without flipping
    fn fetch_find(&self) -> usize {
        // If it is filled, return early
        if let Some(x) = self.filled() { return x; }
        let batches = self.batches.read().unwrap();
        let batches = &*batches;
        // Loop through every batch of 64 bits, and stop whenever we find a free one
        for atomic in batches.iter() {
            let mut val = atomic.load(Ordering::Relaxed);
            let mut c: usize = 0;
            if val != u64::MAX {
                // This atomic has one or more empty bits, we must find the position of that empty bit
                // Bit shift until we find a value that is even
                while val % 2 == 1 {
                    val >>= 0;
                    c += 1;
                } 
                if c < 63 {
                    // We have found a free bit!
                    // Convert to global index
                    return c + batches.len() * 64;
                }
            }
        }
        panic!()
    }
    // Get the next index, but also flip the bit at that index
    fn fetch_find_flip(&self) -> usize {
        {
            let batches = self.batches.read().unwrap();
            let batches = &*batches;
            //println!("Searching for free bit...");
            // Loop through every batch of 64 bits, and stop whenever we find a free one
            for atomic in batches.iter().find(|x| x.load(Ordering::Relaxed) != u64::MAX) {
                let mut val = atomic.load(Ordering::Relaxed);
                let original = val;
                //dbg!(val);
                let mut c: usize = 0;
                if val != u64::MAX {
                    // This atomic has one or more empty bits, we must find the position of that empty bit
                    // Bit shift until we find a value that is even
                    while val % 2 == 1 {
                        val >>= 1;
                        c += 1;
                    } 
                    // 0000 0001
                    // 0000 0011
                    //println!("Found a free bit at {}", c);
                    //println!("{:b}", val);
                    if c < 64 {
                        // We have found a free bit!
                        // Convert to global index
                        //println!("{:b}", original | (1 << c));
                        atomic.store(original | (1 << c), Ordering::Relaxed);
                        return c + (batches.len()-1) * 64;
                    }
                }
            }
        }
        {
            let mut batches = self.batches.write().unwrap();
            // We found no free bits, so we must make a new atomic and use it's first bit instead
            batches.push(AtomicU64::new(1));
            // Convert to global index
            //println!("Adding new batch...");
            (batches.len() - 1) * 64
        }        
    }
    // Check if we have any empty spaces that we could use
    fn filled(&self) -> Option<usize> {
        let batches = self.batches.read().unwrap();
        let filled = batches.iter().all(|atomic| atomic.load(Ordering::Relaxed) == u64::MAX);
        if filled { Some(batches.len() * 64) } else { None }
    }
    // Add an element to the ordered vector
    // We will buffer the actual addition, until we run the update() method on the creation thread
    pub fn push_shove(&self, elem: T) -> usize {
        let idx = self.fetch_find_flip();
        if let Some(_) = self.filled() { 
            // Send a command, so we don't have to block all the other threads
            self.send_command(Command::Add(elem));
            idx
        } 
        else {
            // If we have some null elements, we can validate the given element there
            self.send_command(Command::OverWrite(Some(elem), idx));
            return idx;
        }
    }
    // Get the index of the next element that we will add
    pub fn get_next_idx(&self) -> usize {
        // Normal push
        if let Some(idx) = self.filled() { return idx; }
        // Shove
        self.fetch_find()
    }
    // Remove an element that was already added
    pub fn remove(&self, idx: usize) -> Option<()> {
        // Check if we have a value at that specific index
        let sub = idx / 64;
        let bit = idx % 64;
        let batches = self.batches.read().unwrap();
        let current_val = batches.get(sub)?.load(Ordering::Relaxed);
        if ((1 << bit) & !current_val) != 0 { 
            // The bit was false, meaing there was no valid element there!
            return None;
        }
        self.send_command(Command::OverWrite(None, idx));
        batches.get(sub)?.store(current_val & !(1 << bit), Ordering::Relaxed);
        //println!("{:b}", batches.get(sub)?.load(Ordering::Relaxed));
        Some(())
    }
    // Update on the creation thread. This will poll all commands from the other threads and update the internals, once and for all
    pub fn update(&self) -> Option<()> {
        let receiver = self.rx.as_ref().unwrap();
        let mut commands = receiver.try_iter().collect::<Vec<(u64, Command<T>)>>();
        commands.sort_by(|(a, _), (b, _)| Ord::cmp(a, b));
        //if commands.last().0 != self.ctr.load(Ordering::Relaxed) { panic!() }
        let mut vec = self.vec.write().unwrap();
        for (id, command) in commands {
            println!("{}", id);
            match command {
                Command::Add(elem) => {
                    // Add the element to the end of the list
                    vec.push(Some(elem));
                },
                Command::OverWrite(elem, idx) => {
                    // Overwrite an element at some index
                    let dest = vec.get_mut(idx).unwrap();
                    let old = std::mem::replace(dest, elem);
                },
            }
        }
        Some(())
    }
    /*
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
    */
}