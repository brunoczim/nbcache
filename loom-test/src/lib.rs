use std::collections::HashMap;

use loom::sync::{Arc, Mutex, mpsc};
use nbcache::{
    Cache,
    of::OfCache,
    transforming::{TransformInto, TransformingCache},
};
use rand::random;
use uuid::Uuid;

pub fn generate_key() -> Uuid {
    Uuid::new_v4()
}

pub fn generate_value() -> u64 {
    random()
}

pub fn open(cache_size: usize) -> (ClientSide, Arc<WorkerSide>) {
    let (tx, rx) = mpsc::channel();
    let client = ClientSide {
        source_of_truth: HashMap::new(),
        get_errors: 0,
        delete_errors: 0,
        tx,
    };
    let worker = WorkerSide {
        cold_storage: Mutex::new(HashMap::new()),
        cache: TransformingCache::new(OfCache::with_empty(
            cache_size,
            Uuid::nil().encode(),
        )),
        rx: Mutex::new(rx),
    };
    (client, Arc::new(worker))
}

#[derive(Debug)]
pub enum Request {
    Put(Uuid, u64, mpsc::Sender<()>),
    Get(Uuid, mpsc::Sender<Option<u64>>),
    Delete(Uuid, mpsc::Sender<bool>),
}

#[derive(Debug)]
pub struct ClientSide {
    source_of_truth: HashMap<Uuid, u64>,
    get_errors: u64,
    delete_errors: u64,
    tx: mpsc::Sender<Request>,
}

impl ClientSide {
    pub fn get_errors(&self) -> u64 {
        self.get_errors
    }

    pub fn delete_errors(&self) -> u64 {
        self.delete_errors
    }

    pub fn put(&self, key: Uuid, value: u64) -> PutCall {
        let (tx, rx) = mpsc::channel();
        self.tx.send(Request::Put(key, value, tx)).unwrap();
        PutCall { wait: rx, key, value }
    }

    pub fn get(&self, key: Uuid) -> GetCall {
        let (tx, rx) = mpsc::channel();
        self.tx.send(Request::Get(key, tx)).unwrap();
        GetCall { wait: rx, key }
    }

    pub fn delete(&self, key: Uuid) -> DeleteCall {
        let (tx, rx) = mpsc::channel();
        self.tx.send(Request::Delete(key, tx)).unwrap();
        DeleteCall { wait: rx, key }
    }
}

#[derive(Debug)]
pub struct PutCall {
    key: Uuid,
    value: u64,
    wait: mpsc::Receiver<()>,
}

impl PutCall {
    pub fn finish(self, client: &mut ClientSide) {
        self.wait.recv().unwrap();
        client.source_of_truth.insert(self.key, self.value);
    }
}

#[derive(Debug)]
pub struct GetCall {
    key: Uuid,
    wait: mpsc::Receiver<Option<u64>>,
}

impl GetCall {
    pub fn finish(self, client: &mut ClientSide) {
        let answer = self.wait.recv().unwrap();
        if client.source_of_truth.get(&self.key).copied() != answer {
            client.get_errors += 1;
        }
    }
}

#[derive(Debug)]
pub struct DeleteCall {
    key: Uuid,
    wait: mpsc::Receiver<bool>,
}

impl DeleteCall {
    pub fn finish(self, client: &mut ClientSide) {
        let answer = self.wait.recv().unwrap();
        if client.source_of_truth.remove(&self.key).is_some() != answer {
            client.delete_errors += 1;
        }
    }
}

#[derive(Debug)]
pub struct WorkerSide {
    cache: TransformingCache<Uuid, u64, OfCache<4, 2>>,
    cold_storage: Mutex<HashMap<Uuid, u64>>,
    rx: Mutex<mpsc::Receiver<Request>>,
}

impl WorkerSide {
    pub fn serve(&self) {
        while self.serve_one() {}
    }

    pub fn serve_one(&self) -> bool {
        let Ok(request) = self.rx.lock().unwrap().recv() else {
            return false;
        };

        match request {
            Request::Put(key, value, callback) => {
                self.cache.put(key, value);
                self.cold_storage.lock().unwrap().insert(key, value);
                _ = callback.send(());
            },
            Request::Get(key, callback) => {
                let answer = self.cache.get(key).or_else(|| {
                    self.cold_storage.lock().unwrap().get(&key).copied()
                });
                _ = callback.send(answer);
            },
            Request::Delete(key, callback) => {
                let deleted = self.cache.delete(key)
                    | self.cold_storage.lock().unwrap().remove(&key).is_some();
                _ = callback.send(deleted);
            },
        }

        true
    }
}
