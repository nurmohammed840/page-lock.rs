// #![doc = include_str!("../README.md")]

mod mutex;
mod rw_lock;

use dashmap::DashMap;
use std::{
    future::Future,
    hash::Hash,
    pin::Pin,
    task::Waker,
    task::{Context, Poll},
};

pub use mutex::{Mutex, WriteGuard};
pub use rw_lock::{ReadGuard, RwLock};
