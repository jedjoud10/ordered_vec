pub mod test {
    use crate::{ordered_vec::OrderedVec, shareable_ordered_vec::ShareableOrderedVec};
    use std::{collections::HashMap, sync::Arc, thread::JoinHandle};
    // Test the speed of the ordered vec
    #[test]
    pub fn speed_test() {
        const N: usize = 100_000;
        let mut hashmap = HashMap::<usize, u64>::default();
        let mut ordered_vec = OrderedVec::<u64>::default();
        // Compare a Rust HashMap to my SmartList collection

        // Adding ordered elements
        let i = std::time::Instant::now();
        for x in 0..N {
            hashmap.insert(x, x as u64);
        }
        let x = i.elapsed().as_micros();
        println!("Add HashMap : {}μ", x);

        let i = std::time::Instant::now();
        for x in 0..N {
            ordered_vec.push_shove(x as u64);
        }
        let z = i.elapsed().as_micros();
        println!(
            "Add Ordered Vec: {}μ, {}% faster than HashMap",
            z,
            (x as f32 / z as f32) * 100.0
        );

        let i = std::time::Instant::now();
        for x in 0..(N / 2) {
            hashmap.remove(&x);
        }
        let x = i.elapsed().as_micros();
        println!("Remove HashMap: {}μ", i.elapsed().as_micros());

        let i = std::time::Instant::now();
        for x in 0..(N / 2) {
            ordered_vec.remove(x);
        }
        let z = i.elapsed().as_micros();
        println!(
            "Remove Ordered Vec: {}μ, {}% faster than HashMap",
            i.elapsed().as_micros(),
            (x as f32 / z as f32) * 100.0
        );

        let i = std::time::Instant::now();
        for x in 0..N {
            hashmap.insert(x, x as u64);
        }
        let x = i.elapsed().as_micros();
        println!("Add HashMap : {}μ", x);

        let i = std::time::Instant::now();
        for x in 0..N {
            ordered_vec.push_shove(x as u64);
        }
        let z = i.elapsed().as_micros();
        println!(
            "Add Ordered Vec: {}μ, {}% faster than HashMap",
            z,
            (x as f32 / z as f32) * 100.0
        );
    }
    // An actual unit test to check the order of elements in the collection
    #[test]
    pub fn test() {
        let mut vec = OrderedVec::<i32>::default();
        // Add a few
        let idx_5 = vec.push_shove(5);
        let idx_2 = vec.push_shove(2);
        let idx_6 = vec.push_shove(6);

        // Remove the element 2, and the index for the two other elements should stay the same
        vec.remove(idx_2);

        assert_eq!(vec[idx_5], 5);
        assert_eq!(vec[idx_6], 6);
        assert_eq!(vec.get(idx_2), None);
        // The count should be 2 now
        assert_eq!(vec.count(), 2);

        // Now, we will add another element, and it's index should be the same as idx_2 (Since we re-use deleted indices)
        let idx_9 = vec.push_shove(9);
        assert_eq!(idx_9, idx_2);
    }
    // My drain and iter test
    #[test]
    pub fn iter_test() {
        // Iter test
        let mut vec = OrderedVec::<i32>::default();
        vec.push_shove(0);
        vec.push_shove(1);
        vec.push_shove(2);

        for (index, elem) in vec.iter_indexed() {
            assert_eq!(index, *elem as usize);
        }

        // My drain test
        let mut vec = OrderedVec::<i32>::default();
        vec.push_shove(0);
        vec.push_shove(1);
        vec.push_shove(2);
        vec.push_shove(3);
        vec.push_shove(4);
        let mut removed = vec.my_drain(|index, val| val % 2 == 0);
        assert_eq!(removed.next(), Some((0, 0)));
        assert_eq!(removed.next(), Some((2, 2)));
        assert_eq!(removed.next(), Some((4, 4)));
    }
    // Clearing test
    #[test]
    pub fn clear_test() {
        let mut vec = OrderedVec::<i32>::default();
        vec.push_shove(0);
        vec.push_shove(1);
        vec.push_shove(2);
        assert_eq!(vec.count(), 3);
        // Clear the vector
        let cleared = vec.clear();
        assert_eq!(cleared, vec![Some(0), Some(1), Some(2)]);

        assert_eq!(vec.count(), 0);
        assert_eq!(vec.count_invalid(), 3);
        vec.push_shove(0);
        vec.push_shove(1);
        vec.push_shove(2);
        vec.push_shove(4);
        vec.push_shove(5);
        vec.push_shove(6);
        assert_eq!(
            vec.vec,
            vec![Some(2), Some(1), Some(0), Some(4), Some(5), Some(6)]
        )
    }
    // Test out the shareable ordered vec
    #[test]
    pub fn shareable_test() {
        let mut vec = ShareableOrderedVec::<i32>::default();
        vec.insert(0, 0);
        vec.insert(2, 2);
        vec.insert(6, 4);  
        vec.init_update();
        let shareable = vec.get_shareable();
        dbg!(&vec);
        // Make a simple channel so we can receive at what location we must insert the elements
        let (tx, rx) = std::sync::mpsc::channel::<(usize, i32)>();

        let tx = tx.clone();
        let thread_join_handles = (0..10)
        .map(|_x| {
            // Create a thread
            let vec = shareable.clone();
            let tx = tx.clone();
            std::thread::spawn(move || {
                // Change the bitfield a ton of times
                for i in 0..10 {
                    let elem_index = vec.get_next_id_increment();
                    println!("Next ID: '{}'. Element is: '{}'", elem_index, i + _x * 10);
                    tx.send((elem_index, i + _x * 10)).unwrap();
                }
            })
        })
        .collect::<Vec<JoinHandle<()>>>();
        
        // Join up all the threads
        for x in thread_join_handles {
            x.join().unwrap();
        }

        vec.finish_update();
        // Receive all the messages, and apply them
        for (idx, elem) in rx.try_iter() {
            vec.insert(idx, elem);
        }   

        dbg!(vec);
    }
}
