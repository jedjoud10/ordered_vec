pub mod test {
    use std::{collections::HashMap, thread::JoinHandle};
    use crate::{ordered_vec::OrderedVec, half_concurrent_ordered_vec::HalfConcurrentOrderedVec};
    // Test the speed of the ordered vec
    #[test]
    pub fn speed_test() {
        const N: usize = 10_000;
        let mut hashmap = HashMap::<usize, u64>::default();
        let mut ordered_vec = HalfConcurrentOrderedVec::<u64>::default();
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
        println!("Add Ordered Vec: {}μ, {}% faster than HashMap", z, (x as f32 / z as f32) * 100.0);
    
        let i = std::time::Instant::now();
        for x in 0..(N/2) {
            hashmap.remove(&x);
        }
        let x = i.elapsed().as_micros();
        println!("Remove HashMap: {}μ", i.elapsed().as_micros());

        let i = std::time::Instant::now();
        for x in 0..(N/2) {
            ordered_vec.remove(x);
        }
        let z = i.elapsed().as_micros();
        println!("Remove Ordered Vec: {}μ, {}% faster than HashMap", i.elapsed().as_micros(), (x as f32 / z as f32) * 100.0);   

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
        println!("Add Ordered Vec: {}μ, {}% faster than HashMap", z, (x as f32 / z as f32) * 100.0);
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

        struct CustomStruct();
        let mut vec = OrderedVec::<CustomStruct>::default();
    }
    // Test the Half-Concurrent Ordered Vec
    #[test]
    pub fn test_concurrent() {
        use std::sync::Arc;
        let mut vec = HalfConcurrentOrderedVec::<i32>::default();
        let arc = Arc::new(vec);
        // Create a thread
        let x = (0..5).into_iter().map(|x| {            
            let arc2 = arc.clone();
            std::thread::spawn(move || {
                let vec = arc2;
                for x in 0..1 {
                    let idx = vec.push_shove(0);
                    println!("Added at {}", idx);
                }
                /*
                println!("Removing value at idx 5");
                vec.remove(5).unwrap();
                let idx = vec.push_shove(0);
                println!("Added at {}", idx);
                */
            })
        }).collect::<Vec<JoinHandle<()>>>();
        for y in x { y.join().unwrap(); }
        arc.as_ref().update();
        
    }
}