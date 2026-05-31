#[cfg(not(feature = "loom"))]
use std::{sync::Arc, thread};

#[cfg(feature = "loom")]
use loom::{sync::Arc, thread};

use loom_test::BuildSipHasher;
use nbcache::{Cache, of::OfCacheWith, transforming::TransformingCache};

#[test]
fn atomic_writes_3_threads() {
    let do_test = || {
        let hasher = BuildSipHasher::new(3, 31);
        let cache = Arc::new(TransformingCache::new(OfCacheWith::with_hasher(
            127, hasher,
        )));
        let key = 0x0bcd_fe12_3456_7890_1324_f315_3433_7897_u128;
        let values = [
            0x33cf_2e13_0123_3456_u64,
            12341234567891012_u64,
            9878767656545434323_u64,
        ];

        let thread_0 = thread::spawn({
            let cache = cache.clone();
            move || {
                cache.put(key, values[0]);
            }
        });
        let thread_1 = thread::spawn({
            let cache = cache.clone();
            move || {
                cache.put(key, values[1]);
            }
        });

        cache.put(key, values[2]);

        thread_0.join().unwrap();
        thread_1.join().unwrap();

        let content = cache.get(key).unwrap();

        assert!(values.contains(&content), "Actual: {content}");
    };

    #[cfg(feature = "loom")]
    loom::model(do_test);

    #[cfg(not(feature = "loom"))]
    do_test();
}

#[test]
fn atomic_delete_1_write_2_threads() {
    let do_test = || {
        let hasher = BuildSipHasher::new(3, 31);
        let cache = Arc::new(TransformingCache::new(OfCacheWith::with_hasher(
            127, hasher,
        )));
        let key = 0x0bcd_fe12_3456_7890_1324_f315_3433_7897_u128;
        let values = [0x33cf_2e13_0123_3456_u64, 12341234567891012_u64];

        let thread_0 = thread::spawn({
            let cache = cache.clone();
            move || {
                cache.put(key, values[0]);
            }
        });
        let thread_1 = thread::spawn({
            let cache = cache.clone();
            move || {
                cache.put(key, values[1]);
            }
        });

        cache.delete(key);

        thread_0.join().unwrap();
        thread_1.join().unwrap();

        let maybe_content = cache.get(key);

        assert!(
            maybe_content.is_none_or(|content| values.contains(&content)),
            "Actual: {maybe_content:?}"
        );
    };

    #[cfg(feature = "loom")]
    loom::model(do_test);

    #[cfg(not(feature = "loom"))]
    do_test();
}

#[test]
fn atomic_delete_2_write_1_threads() {
    let do_test = || {
        let hasher = BuildSipHasher::new(3, 31);
        let cache = Arc::new(TransformingCache::new(OfCacheWith::with_hasher(
            127, hasher,
        )));
        let key = 0x0bcd_fe12_3456_7890_1324_f315_3433_7897_u128;
        let values = [0x33cf_2e13_0123_3456_u64, 12341234567891012_u64];

        let thread_0 = thread::spawn({
            let cache = cache.clone();
            move || {
                cache.put(key, values[0]);
            }
        });
        let thread_1 = thread::spawn({
            let cache = cache.clone();
            move || {
                cache.delete(key);
            }
        });

        cache.delete(key);

        thread_0.join().unwrap();
        thread_1.join().unwrap();

        let maybe_content = cache.get(key);

        assert!(
            maybe_content.is_none_or(|content| values.contains(&content)),
            "Actual: {maybe_content:?}"
        );
    };

    #[cfg(feature = "loom")]
    loom::model(do_test);

    #[cfg(not(feature = "loom"))]
    do_test();
}
