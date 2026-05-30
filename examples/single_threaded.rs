use nbcache::raw::OfCache;

fn main() {
    let cache = OfCache::<1, 1>::new(7);

    println!("Putting data into the cache");
    cache.put([123], [0]);
    cache.put([456], [1]);
    cache.put([789], [2]);
    cache.put([12], [3]);
    cache.put([345], [4]);
    cache.delete([12]);

    println!("Getting data from the cache");
    println!("{:?}", cache.get([123]));
    println!("{:?}", cache.get([456]));
    println!("{:?}", cache.get([789]));
    println!("{:?}", cache.get([12]));
    println!("{:?}", cache.get([345]));
    println!("{:?}", cache.get([678]));
    println!("{:?}", cache.get([901]));
}
