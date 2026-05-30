use std::array;

use loom::thread;
use loom_test::{generate_key, generate_value, open};

#[test]
fn same_weight_for_all_operations_sparse_few_threads() {
    loom::model(|| {
        let worker_count = 4;
        let (mut client, worker) = open(131071);
        let mut worker_handles = Vec::with_capacity(worker_count);

        for _ in 0 .. worker_count {
            let worker = worker.clone();
            worker_handles.push(thread::spawn(move || worker.serve()));
        }

        const CONCURRENCY: usize = 4;

        for _ in 0 .. 1 {
            let keys: [_; CONCURRENCY] = array::from_fn(|_| generate_key());
            let values: [_; CONCURRENCY] = array::from_fn(|_| generate_value());
            let puts: [_; CONCURRENCY] =
                array::from_fn(|i| client.put(keys[i], values[i]));
            let gets: [_; CONCURRENCY] =
                array::from_fn(|i| client.get(keys[i]));
            let deletes: [_; CONCURRENCY] =
                array::from_fn(|i| client.delete(keys[i]));

            puts.into_iter().for_each(|req| req.finish(&mut client));
            gets.into_iter().for_each(|req| req.finish(&mut client));
            deletes.into_iter().for_each(|req| req.finish(&mut client));
        }

        for handle in worker_handles {
            handle.join().unwrap();
        }
    });
}
