#[cfg(not(feature = "loom"))]
use std::{sync::Arc, thread};

#[cfg(feature = "loom")]
use loom::{sync::Arc, thread};

use loom_test::BuildSipHasher;
use nbcache::{Cache, of::OfCacheWith, transforming::TransformingCache};

#[test]
fn atomic_reads_3_threads() {
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
                cache.get(key)
            }
        });
        let thread_1 = thread::spawn({
            let cache = cache.clone();
            move || {
                cache.put(key, values[1]);
                cache.get(key)
            }
        });

        cache.put(key, values[2]);

        let result = cache.get(key).unwrap();

        let result_0 = thread_0.join().unwrap().unwrap();
        let result_1 = thread_1.join().unwrap().unwrap();

        assert!(values.contains(&result), "Actual: {result}");
        assert!(values.contains(&result_0), "Actual: {result_0}");
        assert!(values.contains(&result_1), "Actual: {result_1}");
    };

    #[cfg(feature = "loom")]
    loom::model(do_test);

    #[cfg(not(feature = "loom"))]
    do_test();
}
