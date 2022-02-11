use page_lock::RwLock;
use tokio_test::{assert_ready, task::spawn};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;
// #[cfg(target_arch = "wasm32")]
// wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

macro_rules! assert_pending {
    ($result: expr) => {
        assert!(matches!($result, std::task::Poll::Pending))
    };
}

#[test]
fn read_shared() {
    let rwlock = RwLock::new();

    let mut t1 = spawn(rwlock.read(0));
    let _g1 = assert_ready!(t1.poll());
    let mut t2 = spawn(rwlock.read(0));
    assert_ready!(t2.poll());
}

// When there is an active shared owner, exclusive access should not be possible
#[test]
fn write_shared_pending() {
    let rwlock = RwLock::new();
    let mut t1 = spawn(rwlock.read(0));
    let _g1 = assert_ready!(t1.poll());
    let mut t2 = spawn(rwlock.write(0));
    assert_pending!(t2.poll());
}

// When there is an active exclusive owner, subsequent exclusive access should not be possible
#[test]
fn read_exclusive_pending() {
    let rwlock = RwLock::new();
    let mut t1 = spawn(rwlock.write(0));

    let _g1 = assert_ready!(t1.poll());
    let mut t2 = spawn(rwlock.read(0));
    assert_pending!(t2.poll());
}

// When there is an active exclusive owner, subsequent exclusive access should not be possible
#[test]
fn write_exclusive_pending() {
    let rwlock = RwLock::new();
    let mut t1 = spawn(rwlock.write(0));

    let _g1 = assert_ready!(t1.poll());
    let mut t2 = spawn(rwlock.write(0));
    assert_pending!(t2.poll());
}

// When there is an active shared owner, exclusive access should be possible after shared is dropped
#[test]
fn write_shared_drop() {
    let rwlock = RwLock::new();
    let mut t1 = spawn(rwlock.read(0));

    let g1 = assert_ready!(t1.poll());
    let mut t2 = spawn(rwlock.write(0));
    assert_pending!(t2.poll());
    drop(g1);
    assert!(t2.is_woken());
    assert_ready!(t2.poll());
}

// when there is an active shared owner, and exclusive access is triggered,
// subsequent shared access should not be possible as write gathers all the available semaphore permits
#[test]
fn write_read_shared_pending() {
    let rwlock = RwLock::new();
    let mut t1 = spawn(rwlock.read(0));
    let _g1 = assert_ready!(t1.poll());

    let mut t2 = spawn(rwlock.read(0));
    assert_ready!(t2.poll());

    let mut t3 = spawn(rwlock.write(0));
    assert_pending!(t3.poll());

    let mut t4 = spawn(rwlock.read(0));
    assert_pending!(t4.poll());
}

// when there is an active shared owner, and exclusive access is triggered,
// reading should be possible after pending exclusive access is dropped
#[test]
fn write_read_shared_drop_pending() {
    let rwlock = RwLock::new();
    let mut t1 = spawn(rwlock.read(0));
    let _g1 = assert_ready!(t1.poll());

    let mut t2 = spawn(rwlock.write(0));
    assert_pending!(t2.poll());

    let mut t3 = spawn(rwlock.read(0));
    assert_pending!(t3.poll());
    drop(t2);

    assert!(t3.is_woken());
    assert_ready!(t3.poll());
}

#[cfg(not(target_arch = "wasm32"))]
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn multithreaded() {
    use futures::stream::{self, StreamExt};
    use std::sync::Arc;
    use tokio::sync::Barrier;
    static mut C: u32 = 0;
    let barrier = Arc::new(Barrier::new(5));
    let rwlock = Arc::new(RwLock::new());
    let rwclone1 = rwlock.clone();
    let rwclone2 = rwlock.clone();
    let rwclone3 = rwlock.clone();
    let rwclone4 = rwlock.clone();
    let b1 = barrier.clone();
    tokio::spawn(async move {
        stream::iter(0..1000)
            .for_each(move |_| {
                let rwlock = rwclone1.clone();
                async move {
                    let _guard = rwlock.write(0).await;
                    unsafe { C += 2 };
                }
            })
            .await;
        b1.wait().await;
    });
    let b2 = barrier.clone();
    tokio::spawn(async move {
        stream::iter(0..1000)
            .for_each(move |_| {
                let rwlock = rwclone2.clone();
                async move {
                    let _guard = rwlock.write(0).await;
                    unsafe { C += 3 };
                }
            })
            .await;
        b2.wait().await;
    });
    let b3 = barrier.clone();
    tokio::spawn(async move {
        stream::iter(0..1000)
            .for_each(move |_| {
                let rwlock = rwclone3.clone();
                async move {
                    let _guard = rwlock.write(0).await;
                    unsafe { C += 5 };
                }
            })
            .await;
        b3.wait().await;
    });
    let b4 = barrier.clone();
    tokio::spawn(async move {
        stream::iter(0..1000)
            .for_each(move |_| {
                let rwlock = rwclone4.clone();
                async move {
                    let _guard = rwlock.write(0).await;
                    unsafe { C += 7 };
                }
            })
            .await;
        b4.wait().await;
    });
    barrier.wait().await;
    let _g = rwlock.read(0).await;
    assert_eq!(unsafe { C }, 17_000);
}

// // ========================================================================================

#[test]
fn smoke() {
    futures::executor::block_on(async {
        let lock = RwLock::new();
        drop(lock.read(0).await);
        drop(lock.write(0).await);
        drop((lock.read(0).await, lock.read(0).await));
        drop(lock.write(0).await);
    });
}

#[cfg(not(target_arch = "wasm32"))]
#[tokio::test]
async fn contention() {
    const N: u32 = 10;
    const M: usize = 1000;
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let tx = std::sync::Arc::new(tx);
    let rw = std::sync::Arc::new(RwLock::new());
    // Spawn N tasks that randomly acquire the lock M times.
    for _ in 0..N {
        let tx = tx.clone();
        let rw = rw.clone();
        tokio::spawn(async move {
            for _ in 0..M {
                if fastrand::u32(..N) == 0 {
                    drop(rw.write(0).await);
                } else {
                    drop(rw.read(0).await);
                }
            }
            tx.send(()).unwrap();
        });
    }
    for _ in 0..N {
        rx.recv().await.unwrap();
    }
}
