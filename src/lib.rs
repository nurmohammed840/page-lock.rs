#![doc = include_str!("../README.md")]

mod mutex;
mod rw_lock;

use c_map::HashMap;

use std::{
    future::Future,
    hash::Hash,
    pin::Pin,
    task::Waker,
    task::{Context, Poll},
};

pub use mutex::{Mutex, WriteGuard};
pub use rw_lock::{ReadGuard, RwLock};


// #[tokio::test]
// async fn test_rwlock2() {
//     let m: &'static RwLock<u8> = Box::leak(Box::new(RwLock::new()));
//     let v = vec![
//         tokio::spawn(async move {
//             let _a = m.read(0).await;
//             tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
//             println!("read 2");
//         }),
//         tokio::spawn(async move {
//             let _b = m.read(0).await;
//             println!("read 1");
//         }),
//         tokio::spawn(async move {
//             let _c = m.write(0).await;
//             println!("write 1");
//         }),
//         tokio::spawn(async move {
//             let _d = m.write(0).await;
//             println!("write 2");
//         }),
//         tokio::spawn(async move {
//             let _e = m.read(0).await;
//             println!("read");
//         }),
//         tokio::spawn(async move {
//             let _f = m.write(0).await;
//             println!("write");
//         }),
//     ];
//     for v in v {
//         v.await.unwrap();
//     }
// }

// #[tokio::test]
// async fn test_s() {
//     let rwlock = RwLock::new();
//     let _r = rwlock.write(0).await;
//     tokio::time::timeout(tokio::time::Duration::from_millis(100), async {
//         println!("acquire.");
//         let _ra = rwlock.read(0).await;
//         println!("release");
//     })
//     .await
//     .unwrap();
// }
