use page_lock::Mutex;
use std::sync::Arc;
use tokio_test::assert_ready;
use tokio_test::task::spawn;

macro_rules! assert_pending {
    ($result: expr) => {
        assert!(matches!($result, std::task::Poll::Pending))
    };
}

#[test]
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

#[tokio::test]
async fn aborted_future_1() {
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
}

/// This test is similar to `aborted_future_1` but this time the
/// aborted future is waiting for the lock.
#[tokio::test]
async fn aborted_future_2() {
    use std::time::Duration;
    use tokio::time::timeout;

    let m1: Arc<Mutex<usize>> = Arc::new(Mutex::new());
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
            .unwrap();
        }
    }
    // This should succeed as there is no lock left for the mutex.
    timeout(Duration::from_millis(1u64), async move {
        m1.lock(0).await;
    })
    .await
    .expect("Mutex is locked");
}