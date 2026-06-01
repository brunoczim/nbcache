use std::sync::Arc;

pub mod of;
pub mod transforming;

#[cfg(feature = "testing")]
pub mod testing;

pub trait CacheType {}

pub trait Cache {
    type Key;
    type Value;
    type Type: CacheType;

    fn put(&self, key: Self::Key, value: Self::Value);

    fn get(&self, key: Self::Key) -> Option<Self::Value>;

    fn delete(&self, key: Self::Key) -> bool;
}

impl<C> Cache for &'_ C
where
    C: Cache + ?Sized,
{
    type Type = C::Type;
    type Key = C::Key;
    type Value = C::Value;

    fn put(&self, key: Self::Key, value: Self::Value) {
        (**self).put(key, value);
    }

    fn get(&self, key: Self::Key) -> Option<Self::Value> {
        (**self).get(key)
    }

    fn delete(&self, key: Self::Key) -> bool {
        (**self).delete(key)
    }
}

impl<C> Cache for Arc<C>
where
    C: Cache + ?Sized,
{
    type Type = C::Type;
    type Key = C::Key;
    type Value = C::Value;

    fn put(&self, key: Self::Key, value: Self::Value) {
        (**self).put(key, value);
    }

    fn get(&self, key: Self::Key) -> Option<Self::Value> {
        (**self).get(key)
    }

    fn delete(&self, key: Self::Key) -> bool {
        (**self).delete(key)
    }
}
