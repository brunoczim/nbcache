//! Reproducer for the concurrent-writer livelock in `nbcache::raw` (see the
//! `write_pair` comment in `src/raw.rs`).
//!
//! Ignored by default: once a slot wedges, the writer threads hang forever.
//! Rather than hang the whole test binary, this fails fast via a channel
//! timeout. Re-enable once the write path is fixed.

use std::{
    sync::{Arc, mpsc},
    thread,
    time::Duration,
};

use nbcache::raw::NbCache;

#[test]
#[ignore = "known concurrent-writer livelock; see write_pair comment in src/raw.rs"]
fn concurrent_writers_wedge_the_slot() {
    // A size-1 cache forces every key onto the same slot, maximizing write
    // contention regardless of the hasher. A single bad interleaving wedges
    // the slot's version permanently, so a wedged writer's `put` never returns;
    // most interleavings resolve, so sustained contention is needed to surface
    // it. Three writers hammering one slot hit it reliably and quickly.
    let cache = Arc::new(NbCache::<1, 1>::new(1));
    let (tx, rx) = mpsc::channel();

    for key in 1u32..=3 {
        let cache = Arc::clone(&cache);
        let tx = tx.clone();
        thread::spawn(move || {
            for _ in 0..100_000 {
                cache.put([key], [key]);
            }
            let _ = tx.send(key);
        });
    }
    drop(tx);

    for _ in 0..3 {
        rx.recv_timeout(Duration::from_secs(10)).expect(
            "a writer never finished its puts within 10s: \
             concurrent-writer livelock in write_pair",
        );
    }
}
