#[cfg(test)]
pub mod test {
    use crate::{shareable_ordered_vec::ShareableOrderedVec, simple::*};
    use std::{
        collections::HashMap,
        sync::{Arc, RwLock},
        thread::JoinHandle,
    };
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
            ordered_vec.remove_index(x);
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
        //dbg!(idx_5);
        //dbg!(idx_2);
        //dbg!(idx_6);
        assert_eq!(vec[idx_5], 5);
        assert_eq!(vec[idx_6], 6);
        assert_eq!(vec.get(idx_2), None);
        // The count should be 2 now
        assert_eq!(vec.count(), 2);

        // Now, we will add another element, and it's index should be the same as idx_2 (Since we re-use deleted indices)
        let idx_9 = vec.push_shove(9);
        //dbg!(from_id(idx_9));
        //dbg!(from_id(idx_2));
        assert_ne!(idx_9, idx_2);
    }
    // My drain and iter test
    #[test]
    pub fn iter_test() {
        // Iter test
        let vec = OrderedVec::<u64>::default();
        //dbg!(vec.push_shove(0_u64 | (0_u64 << 32)));
        //dbg!(vec.push_shove(1_u64 | (0_u64 << 32)));
        //dbg!(vec.push_shove(2_u64 | (0_u64 << 32)));

        for (id, elem) in vec.iter() {
            assert_eq!(id, *elem);
        }

        // My drain test
        let mut vec = OrderedVec::<i32>::default();
        vec.push_shove(0);
        vec.push_shove(1);
        vec.push_shove(2);
        vec.push_shove(3);
        let last = vec.push_shove(4);
        vec.remove(last).unwrap();
        vec.push_shove(4);
        let mut removed = vec.my_drain(|_index, val| val % 2 == 0);
        assert_eq!(removed.next(), Some((0_u64 | (0_u64 << 32), 0)));
        assert_eq!(removed.next(), Some((2_u64 | (0_u64 << 32), 2)));
        assert_eq!(removed.next(), Some((4_u64 | (1_u64 << 32), 4)));
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
        assert_eq!(vec.count_invalid(), 0);
        vec.push_shove(0);
        vec.push_shove(1);
        vec.push_shove(2);
        vec.push_shove(3);
        vec.push_shove(4);
        vec.push_shove(5);
        assert_eq!(vec.count(), 6);
        assert_eq!(vec.count_invalid(), 0);
        let x = vec.into_iter().map(|(_, elem)| elem).collect::<Vec<i32>>();
        assert_eq!(x, vec![0, 1, 2, 3, 4, 5])
    }
    // ID test
    #[test]
    pub fn id_test() {
        let mut vec = OrderedVec::<String>::default();
        let bob_id = vec.push_shove("Bob".to_string());
        assert_eq!(bob_id, 0);
        assert_eq!(vec.get_next_id(), 1_u64);
        assert!(vec.remove(bob_id).is_some());
        let john_id = vec.get_next_id(); // Index: 0, Version: 1
        let john_id2 = vec.push_shove("John".to_string()); // Index: 0, Version: 1
        assert_eq!(john_id, john_id2);
        assert_eq!(john_id2, (0_u64 | (1_u64 << 32)))
    }
    // ID test but for the unversionned version
    #[test]
    pub fn index_unversionned_test() {
        let mut vec = UnversionnedOrderedVec::<String>::default();
        let bob_id = vec.push_shove("Bob".to_string());
        assert_eq!(bob_id, 0);
        assert_eq!(vec.get_next_idx(), 1);
        assert!(vec.remove(bob_id).is_some());
        let john_id = vec.get_next_idx(); // Index: 0
        let john_id2 = vec.push_shove("John".to_string()); // Index: 0
        assert_eq!(john_id, john_id2);
        assert_eq!(john_id2, 0)
    }
    // Test out the shareable ordered vec
    #[test]
    pub fn shareable_test() {
        let mut vec = ShareableOrderedVec::<String>::default();
        vec.insert(0, "Bob".to_string());
        vec.remove(0);
        vec.insert(0_u64 | (1_u64 << 32), "Bob".to_string());
        vec.insert(2, "John".to_string());
        vec.insert(4, "Lina".to_string());
        /*
         */
        // +-------+--------+
        // | Index | Value  |
        // +-------+--------+
        // |     0 | "Bob"  |
        // |     1 | None   |
        // |     2 | "John" |
        // |     3 | None   |
        // |     4 | "Lina" |
        // +-------+--------+
        //dbg!(&vec);
        // Make a simple channel so we can receive at what location we must insert the elements
        let (tx, rx) = std::sync::mpsc::channel::<(u64, String)>();

        let tx = tx;
        let arc = Arc::new(RwLock::new(vec));
        let thread_join_handles = (0..10)
            .map(|_x| {
                // Create a thread
                let arc = arc.clone();
                let tx = tx.clone();
                std::thread::spawn(move || {
                    // Change the bitfield a ton of times
                    for i in 0..10 {
                        let elem_id = arc.read().unwrap().get_next_id_increment();
                        //println!("Next ID: '{}'. Element is: '{}'", elem_id, i + _x * 10);
                        tx.send((elem_id, format!("Number {}", i + _x * 10)))
                            .unwrap();
                    }
                })
            })
            .collect::<Vec<JoinHandle<()>>>();

        // Join up all the threads
        for x in thread_join_handles {
            x.join().unwrap();
        }
        let mut vec = Arc::try_unwrap(arc).unwrap().into_inner().unwrap();

        // Receive all the messages, and apply them
        for (idx, elem) in rx.try_iter() {
            vec.insert(idx, elem);
        }
        //dbg!(vec);
    }
    // An even better shareable test
    #[test]
    pub fn shareable_test2() {
        let mut vec = ShareableOrderedVec::<String>::default();
        vec.insert(0, "Bob".to_string());
        vec.insert(1, "John".to_string());
        vec.insert(2, "Lina".to_string());
        assert_eq!(vec.count(), 3);
        vec.remove(1);
        assert_eq!(vec.count(), 2);
        //dbg!(&vec.missing);

        // Ticky part
        let next_id = vec.get_next_id_increment();
        assert_eq!(next_id, 1 | (1_u64 << 32)); // Versionning moment
        let next_id2 = vec.get_next_id_increment();
        assert_eq!(next_id2, 3);
        vec.insert(next_id, "Boi".to_string());
        vec.insert(next_id2, "Moment".to_string());
        assert_eq!(vec.count(), 4);
        assert_eq!(vec.count_invalid(), 0);
    }
}
