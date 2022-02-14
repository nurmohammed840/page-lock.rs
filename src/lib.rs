#![doc = include_str!("../README.md")]

mod mutex;
mod rw_lock;

pub use mutex::{Mutex, UntilUnlocked, WriteGuard};
pub use rw_lock::{ReadGuard, RwLock};

use std::{
    cell::UnsafeCell,
    future::Future,
    hash::Hash,
    pin::Pin,
    task::Waker,
    task::{Context, Poll},
};

pub(crate) type Map<K, V> = std::sync::Mutex<std::collections::HashMap<K, V>>;
pub(crate) fn new_map<K, V>() -> std::sync::Mutex<std::collections::HashMap<K, V>> {
    std::sync::Mutex::new(std::collections::HashMap::new())
}

pub(crate) enum PollState {
    Init,
    Pending,
    Ready,
}
macro_rules! ret_fut {
    ($state: expr, $body: block) => {
        unsafe {
            match $state {
                PollState::Init => {
                    $body;
                    $state = PollState::Pending;
                }
                PollState::Ready => return Poll::Ready(()),
                _ => {}
            };
            return Poll::Pending;
        }
    };
}
pub(crate) use ret_fut;
