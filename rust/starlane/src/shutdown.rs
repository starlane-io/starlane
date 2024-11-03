use std::future::Future;
use std::pin::Pin;
use once_cell::sync::Lazy;
use std::process;

static SHUTDOWN_HOOK_TX: Lazy<tokio::sync::mpsc::Sender<ShutdownCall>> = Lazy::new(|| {
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    tokio::spawn(async move {
        let mut hooks = vec![];
        while let Some(call) = rx.recv().await {
            match call {
                ShutdownCall::AddHook(tx) => {
                    hooks.push(tx);
                }
                ShutdownCall::Shutdown(code) => {
                    for f in hooks.drain(..) {
                        f.await;
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    process::exit(code);
                }
            }
        }


    });
    tx
});

pub fn add_shutdown_hook(f: Pin<Box<dyn Future<Output=()>+Sync+Send+'static>> )  {
    SHUTDOWN_HOOK_TX.try_send( ShutdownCall::AddHook(f)).unwrap_or_default();
}

pub fn shutdown(code: i32) {
    SHUTDOWN_HOOK_TX.try_send(ShutdownCall::Shutdown(code)).unwrap_or_default();
}


pub enum ShutdownCall  {
    AddHook(Pin<Box<dyn Future<Output=()>+Sync+Send+'static>>),
    Shutdown(i32),
}

