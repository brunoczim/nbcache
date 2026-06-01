use std::sync::Arc;

use benchmark::{BenchmarkConfig, run_once};
use nbcache::{
    of::OfCache,
    testing::{
        CoarseBlockingCache,
        CoarseRwBlockingCache,
        FineBlockingCache,
        FineRwBlockingCache,
    },
};

fn main() {
    let thread_count = 8;
    let key_count = 1_000_000;

    let configs = [
        BenchmarkConfig {
            thread_count,
            key_count,
            put_interval: 15,
            get_interval: 1,
            delete_interval: 17,
        },
        BenchmarkConfig {
            thread_count,
            key_count,
            put_interval: 7,
            get_interval: 1,
            delete_interval: 8,
        },
        BenchmarkConfig {
            thread_count,
            key_count,
            put_interval: 2,
            get_interval: 1,
            delete_interval: 4,
        },
        BenchmarkConfig {
            thread_count,
            key_count,
            get_interval: 1,
            put_interval: 1,
            delete_interval: 1,
        },
    ];

    for config in configs {
        let capacity = 4000037;
        let of_result =
            run_once(config, Arc::new(OfCache::<4, 2>::new(capacity)));
        let fine_blocking_result = run_once(
            config,
            Arc::new(FineBlockingCache::<[u32; 4], [u32; 2]>::new(capacity)),
        );
        let coarse_blocking_result = run_once(
            config,
            Arc::new(CoarseBlockingCache::<[u32; 4], [u32; 2]>::new()),
        );
        let fine_rw_blocking_result = run_once(
            config,
            Arc::new(FineRwBlockingCache::<[u32; 4], [u32; 2]>::new(capacity)),
        );
        let coarse_rw_blocking_result = run_once(
            config,
            Arc::new(CoarseRwBlockingCache::<[u32; 4], [u32; 2]>::new()),
        );
        println!(
            "# put={} ; get={} ; delete={} ;",
            config.put_interval, config.get_interval, config.delete_interval
        );
        println!("ofcache: {:?}", of_result);
        println!("coarse blocking: {:?}", coarse_blocking_result);
        println!("fine blocking: {:?}", fine_blocking_result);
        println!("coarse RW blocking: {:?}", coarse_rw_blocking_result);
        println!("fine RW blocking: {:?}", fine_rw_blocking_result);
        println!();
    }
}
