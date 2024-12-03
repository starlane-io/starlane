use once_cell::sync::Lazy;
use std::future::Future;
use std::pin::Pin;
use std::process;
use std::time::Duration;
use tokio::join;
use tokio::task::JoinSet;

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
                    let mut set = JoinSet::from_iter(hooks);
                    tokio::time::timeout(Duration::from_secs(15u64), async move {
                        while let Some(res) = set.join_next().await {}
                    })
                    .await
                    .unwrap_or_default();
                    process::exit(code);
                }
            }
        }
    });
    tx
});

pub fn add_shutdown_hook(f: Pin<Box<dyn Future<Output = ()> + Sync + Send + 'static>>) {
    SHUTDOWN_HOOK_TX
        .try_send(ShutdownCall::AddHook(f))
        .unwrap_or_default();
}

pub fn shutdown(code: i32) {
    SHUTDOWN_HOOK_TX
        .try_send(ShutdownCall::Shutdown(code))
        .unwrap_or_default();
}

pub fn panic_shutdown<M>(msg: M)
where
    M: AsRef<str>,
{
    eprintln!("{}", msg.as_ref());
    SHUTDOWN_HOOK_TX
        .try_send(ShutdownCall::Shutdown(1))
        .unwrap_or_default();
}

pub enum ShutdownCall {
    AddHook(Pin<Box<dyn Future<Output = ()> + Sync + Send + 'static>>),
    Shutdown(i32),
}
