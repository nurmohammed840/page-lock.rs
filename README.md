[Docs](https://docs.rs/page-lock)

An async library for locking page address.

You may want to use this library to lock spacific page address. (For example database)

#### Example

Add this to your project's `Cargo.toml` file.

```toml
[dependencies]
page-lock = "1"
```

Basic example:

```rust
use page_lock::PageLocker;
use std::sync::Arc;
use std::time::Duration;
use tokio::{spawn, time::sleep};

let locker = Arc::new(PageLocker::new());
let locker1 = locker.clone();
tokio::try_join!(
    spawn(async move {
        let _lock = locker.lock(1);
        println!("(1) Page 1: Locked");
        sleep(Duration::from_secs(3)).await;
        println!("(3) Page 1: Droping lock");
    }),
    spawn(async move {
        println!("(2) Page 1: Waiting for unlock...");
        locker1.unlock(1).await;
        println!("(4) Page 1: Unlocked!");
    })
)
.unwrap();
```