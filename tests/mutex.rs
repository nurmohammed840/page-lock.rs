use futures_lite::future;
use page_lock::Mutex;
use std::sync::Arc;
use tokio_test::assert_ready;
use tokio_test::task::spawn;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test;
// #[cfg(target_arch = "wasm32")]
// wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

macro_rules! assert_pending {
    ($result: expr) => {
        assert!(matches!($result, std::task::Poll::Pending))
    };
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn readiness() {
    let l1 = Arc::new(Mutex::new());
    let l2 = Arc::clone(&l1);
    let mut t1 = spawn(l1.lock(0));
    let mut t2 = spawn(l2.lock(0));

    let g = assert_ready!(t1.poll());

    // We can't now acquire the lease since it's already held in g
    assert_pending!(t2.poll());

    // But once g unlocks, we can acquire it
    drop(g);
    assert!(t2.is_woken());
    assert_ready!(t2.poll());
}

#[test]
fn aborted_future_1() {
    future::block_on(async {
        use std::time::Duration;
        use tokio::time::{interval, timeout};

        let m1: Arc<Mutex<usize>> = Arc::new(Mutex::new());
        {
            let m2 = m1.clone();
            // Try to lock mutex in a future that is aborted prematurely
            timeout(Duration::from_millis(1u64), async move {
                let iv = interval(Duration::from_millis(1000));
                tokio::pin!(iv);
                m2.lock(0).await;
                iv.as_mut().tick().await;
                iv.as_mut().tick().await;
            })
            .await
            .unwrap_err();
        }
        // This should succeed as there is no lock left for the mutex.
        timeout(Duration::from_millis(1u64), async move {
            m1.lock(0).await;
        })
        .await
        .expect("Mutex is locked");
    });
}

/// This test is similar to `aborted_future_1` but this time the
/// aborted future is waiting for the lock.
#[test]
fn aborted_future_2() {
    future::block_on(async {
        use std::time::Duration;
        use tokio::time::timeout;

        let m1 = Arc::new(Mutex::new());
        {
            // Lock mutex
            let _lock = m1.lock(0).await;
            {
                let m2 = m1.clone();
                // Try to lock mutex in a future that is aborted prematurely
                timeout(Duration::from_millis(1u64), async move {
                    m2.lock(0).await;
                })
                .await
                .unwrap_err();
            }
        }
        // This should succeed as there is no lock left for the mutex.
        timeout(Duration::from_millis(1u64), async move {
            m1.lock(0).await;
        })
        .await
        .expect("Mutex is locked");
    });
}

// =====================================================================================

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn smoke() {
    future::block_on(async {
        let m = Mutex::new();
        drop(m.lock(0).await);
        drop(m.lock(0).await);
    });
}

#[test]
fn contention() {
    future::block_on(async {
        static mut C: usize = 0;
        let (tx, rx) = async_channel::unbounded();

        let tx = Arc::new(tx);
        let mutex = Arc::new(Mutex::new());
        let num_tasks = 100;

        for _ in 0..num_tasks {
            let tx = tx.clone();
            let mutex = mutex.clone();

            std::thread::spawn(move || {
                // Create the runtime
                // Execute the future, blocking the current thread until completion
                future::block_on(async {
                    let lock = mutex.lock(0).await;
                    unsafe { C += 1 };
                    tx.send(()).await.unwrap();
                    drop(lock);
                });
            });
        }
        for _ in 0..num_tasks {
            rx.recv().await.unwrap();
        }
        let _lock = mutex.lock(0).await;
        assert_eq!(num_tasks, unsafe { C });
    });
}

// #[tokio::test]
// async fn test_rwlock2() {
//     let m: &'static RwLock<u8> = Box::leak(Box::new(RwLock::new()));
//     let v = vec![
//         // tokio::spawn(async move {
//         //     let _a = m.read(0).await;
//         //     tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
//         //     println!("read 2");
//         // }),
//         // tokio::spawn(async move {
//         //     let _b = m.read(0).await;
//         //     println!("read 1");
//         // }),
//         // tokio::spawn(async move {
//         //     let _c = m.write(0).await;
//         //     println!("write 1");
//         // }),
//         tokio::spawn(async move {
//             println!("write 1 start");
//             let _d = m.write(0, 1).await;
//             tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
//             println!("write 1 end");
//         }),
//         tokio::spawn(async move {
//             println!("read 2 start");
//             let _e = m.read(0).await;
//             println!("read 2 end");
//         }),
//         tokio::spawn(async move {
//             println!("write 11 start");
//             let _f = m.write(0, 11).await;
//             println!("write 11 end");
//         }),
//     ];
//     for v in v {
//         v.await.unwrap();
//     }
// }
