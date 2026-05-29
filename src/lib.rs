//! Non-blocking, fixed-capacity cache for keys and values made of `u32` words.
//!
//! The cache is *direct-mapped*: each key hashes to exactly one slot, and
//! writing a key that collides with an existing one evicts the previous
//! occupant. Reads and writes never block; concurrent access is mediated by a
//! per-slot versioned, double-buffered scheme (see [`raw`]).
//!
//! # Example
//!
//! ```
//! use nbcache::raw::NbCache;
//!
//! let cache = NbCache::<1, 1>::new(16);
//! cache.put([42], [7]);
//! assert_eq!(cache.get([42]), Some([7]));
//!
//! assert!(cache.delete([42]));
//! assert_eq!(cache.get([42]), None);
//! ```

pub mod raw;
