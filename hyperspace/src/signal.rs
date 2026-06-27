//! The point of the signaling system is to be able to compose a chain of 'stations'
//! that can process a [Signal] which can be a call/return pattern or probes or reports on
//! the state of the signal chain.

use async_trait::async_trait;
use dashmap::DashMap;
use serde_derive::{Deserialize, Serialize};
use starlane_space::status::StatusReport;
use std::marker::PhantomData;
use std::sync::atomic::AtomicU64;
use thiserror::Error;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Signal<C, R> {
    Call(u64, C),
    Return(u64, R),
    Probe(Probe),
    Report(Report),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Probe {
    Status,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Report {
    Status(StatusReport),
}

#[async_trait]
pub trait SignalTransfer {
    type Call;
    type Return;

    async fn send(&self, signal: Signal<Self::Call, Self::Return>) -> Result<(), SignalErr>;
}

#[derive(Debug, Error)]
pub enum SignalErr {
    /// signal chain is pending some state change (like a connection is trying
    /// to be established to an external database)
    #[error("Signal pending...")]
    Pending,
    /// signal chain does not know what to do and needs intervention
    /// to be unpanicked
    #[error("signal panic")]
    Panic,
    /// signal chain cannot be recovered
    #[error("fatal")]
    Fatal,
}

pub struct Station<Up, Down>
where
    Up: SignalTransfer,
    Down: SignalTransfer,
{
    pub name: String,
    up: Up,
    down: Down,
}

pub struct Exchanger<T>
where
    T: SignalTransfer,
{
    sequence: AtomicU64,
    call_return: DashMap<u64, tokio::sync::oneshot::Sender<T::Return>>,
    transfer: T,
}

impl<T> Exchanger<T>
where
    T: SignalTransfer,
{
    async fn send(&self, signal: Signal<T::Call, T::Return>) -> Result<(), SignalErr> {
        self.transfer.send(signal).await
    }

    async fn call(&self, call: T::Call) -> tokio::sync::oneshot::Receiver<T::Return> {
        let id = self
            .sequence
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.call_return.insert(id, tx);
        let signal = Signal::Call(id, call);
        /// need to handle a Result here... instead of just ignoring it...  More importantly
        /// the signal chain needs to change its status so that it is known to be broken here.
        self.send(signal).await;
        rx
    }
}
