use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    thread,
};

use nbcache::of::OfCache;

fn main() {
    let n_threads = 8;

    let cache = Arc::new(OfCache::<4, 2>::new(131071));
    let get_counter = Arc::new(AtomicU64::new(0));
    let delete_counter = Arc::new(AtomicU64::new(0));

    let mut threads = Vec::with_capacity(n_threads);
    for i in 0 .. n_threads {
        let cache = cache.clone();
        let get_counter = get_counter.clone();
        let delete_counter = delete_counter.clone();
        threads.push(thread::spawn(move || {
            let total_entries = 1024 + i * 2048;

            println!("Thread {i} phase 1 starting...");
            for j in 0 .. total_entries {
                let key = [
                    i.wrapping_add(j),
                    i.wrapping_sub(j),
                    i.wrapping_mul(j),
                    i ^ j,
                ]
                .map(|elem| elem as u32);
                let value = [j.wrapping_sub(i), j | i].map(|elem| elem as u32);
                cache.put(key, value);
            }

            println!("Thread {i} phase 2 starting...");
            for j in 0 .. total_entries {
                let key = [
                    i.wrapping_add(j),
                    i.wrapping_sub(j),
                    i.wrapping_mul(j),
                    i ^ j,
                ]
                .map(|elem| elem as u32);
                if cache.get(key).is_some() {
                    get_counter.fetch_add(1, Ordering::Relaxed);
                }
            }

            println!("Thread {i} phase 3 starting...");
            for j in 0 .. total_entries {
                let k = (i + 1) % n_threads;
                let key = [
                    k.wrapping_add(j),
                    k.wrapping_sub(j),
                    k.wrapping_mul(j),
                    k ^ j,
                ]
                .map(|elem| elem as u32);
                if cache.get(key).is_some() {
                    get_counter.fetch_add(1, Ordering::Relaxed);
                }
            }

            println!("Thread {i} phase 4 starting...");
            for j in 0 .. total_entries {
                let key = [
                    i.wrapping_add(j),
                    i.wrapping_sub(j),
                    i.wrapping_mul(j),
                    i ^ j,
                ]
                .map(|elem| elem as u32);
                if cache.delete(key) {
                    delete_counter.fetch_add(1, Ordering::Relaxed);
                }
            }

            println!("Thread {i} phase 5 starting...");
            for j in 0 .. total_entries {
                let k = (i + 1) % n_threads;
                let key = [
                    k.wrapping_add(j),
                    k.wrapping_sub(j),
                    k.wrapping_mul(j),
                    k ^ j,
                ]
                .map(|elem| elem as u32);
                if cache.delete(key) {
                    delete_counter.fetch_add(1, Ordering::Relaxed);
                }
            }

            println!("Thread {i} leaving...");
        }));
    }

    for thread in threads {
        thread.join().unwrap();
    }

    println!("get() hits: {}", get_counter.load(Ordering::Relaxed));
    println!("delete() hits: {}", delete_counter.load(Ordering::Relaxed));
}
