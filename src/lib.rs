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

pub use mutex::{Mutex, MutexGuard};
pub use rw_lock::{ReadGuard, RwLock};

pub(crate) enum PollState {
    Init,
    Pending,
    Ready,
}

macro_rules! ret_fut {
    ($state: expr, $body: block) => {
        match $state {
            PollState::Init => {
                $body;
                $state = PollState::Pending;
            }
            PollState::Ready => return Poll::Ready(()),
            _ => {}
        };
        return Poll::Pending;
    };
}
pub(crate) use ret_fut;
