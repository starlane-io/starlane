//! The point of the signaling system is to be able to compose a chain of 'stations'
//! that can process a [Signal] which can be a call/return pattern or probes or reports on
//! the state of the signal chain.

use crate::signal::err::TransferErr;
use async_trait::async_trait;
use dashmap::DashMap;
use serde_derive::{Deserialize, Serialize};
use starlane_space::status::{PendingDetail, StatusDetail, StatusReport};
use std::marker::PhantomData;
use std::sync::atomic::AtomicU64;
use futures::SinkExt;
use thiserror::Error;
use tokio::sync::mpsc::error::SendError;
use tokio::sync::oneshot::error::RecvError;

type CallReturnMap<R> = DashMap<u64, tokio::sync::oneshot::Sender<R>>;
type StatusRx = tokio::sync::watch::Receiver<StatusDetail>;
type StatusTx = tokio::sync::watch::Sender<StatusDetail>;

fn status_tx(status: StatusDetail) -> StatusTx {
    let (tx, _) = tokio::sync::watch::channel(status);
    tx
}

static TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SigCall<C>(u64, C);

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SigRet<R>(u64, R);


#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Signal<C, R> {
    Call(SigCall<C>),
    Return(SigRet<R>),
    Probe(Probe),
    Report(Report),
}

impl <C,R> Signal<C,R> {
    pub fn call(id: u64, call: C) -> Self {
        Signal::Call(SigCall(id, call))
    }
    pub fn return_(id: u64, ret: R) -> Self {
        Signal::Return(SigRet(id, ret))
    }

}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Probe {
    Status,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Report {
    Status(StatusReport),
}

struct SignalTransfer<Call, Return> {
    tx: tokio::sync::mpsc::Sender<Signal<Call, Return>>,
    status_tx: tokio::sync::watch::Sender<StatusDetail>,
}

impl<Call, Return> SignalTransfer<Call, Return> {
    fn new(tx: tokio::sync::mpsc::Sender<Signal<Call, Return>>) -> Self {
        let (status_tx, _) = tokio::sync::watch::channel(StatusDetail::Ready);
        Self { tx, status_tx }
    }
    async fn send(&self, signal: Signal<Call, Return>) {
        if let Err(err) = self.tx.send(signal).await {
            if self.tx.is_closed() {
                self.status_tx
                    .send(StatusDetail::Fatal("Signal channel closed".to_string()));
            } else {
                self.status_tx.send(StatusDetail::Panic(
                    "Signal channel not responding".to_string(),
                ));
            }
        }
    }

    fn status_watcher(&self) -> StatusRx {
        self.status_tx.subscribe()
    }
}


struct SignalRx<Call,Return, CallHandle,ReturnHandle> where Call: Send+Sync+'static, Return: Send+Sync+'static,
                                                             CallHandle: CallHandler<Call=Call>+'static, ReturnHandle: ReturnHandler<Return=Return>+'static{
    down: tokio::sync::mpsc::Receiver<Signal<Call,Return>>,
    up: SignalTransfer<Call,Return>,
    status_tx: tokio::sync::watch::Sender<StatusDetail>,
    call_handler: CallHandle,
    return_handler: ReturnHandle,
}

impl <Call,Return, CallHandle,ReturnHandle>SignalRx<Call, Return, CallHandle,ReturnHandle> where Call: Send+Sync+'static, Return: Send+Sync+'static,
                                                                                                    CallHandle: CallHandler<Call=Call>+'static, ReturnHandle: ReturnHandler<Return=Return>+'static{

    pub fn new(rx: tokio::sync::mpsc::Receiver<Signal<Call, Return>>, call_handler: CallHandle, return_handler: ReturnHandle) -> StatusRx {

        let status_tx = status_tx(StatusDetail::Ready);
        let status_rx = status_tx.subscribe();
        let mut runner = Self {
            down: rx,
            status_tx,
            call_handler,
            return_handler
        };
        tokio::spawn(async move {
            runner.start().await
        });
        status_rx
    }
    async fn start(mut self) {
        while let Some(signal) = self.down.recv().await {
            match signal {
                Signal::Call(call) => {
                    self.call_handler.handle_call(call);
                }
                Signal::Return(ret) => {
                    self.return_handler.handle_return(ret);
                }
                Signal::Probe(_) => {}
                Signal::Report(_) => {}
            }
        }
        self.status_tx.send(StatusDetail::Stopped);
    }
}



pub trait CallHandler: Send+Sync {
    type Call;

    fn handle_call(&self, call: SigCall<Self::Call>);
}

pub trait ReturnHandler: Send+Sync {
    type Return;

    fn handle_return(&self, ret: SigRet<Self::Return>);
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

pub struct Station<UpCall, UpRet, DownCall, DownRet> {
    name: String,
    up: SignalTransfer<UpCall, UpRet>,
    down: SignalTransfer<DownCall, DownRet>,
}

pub struct Exchanger<Call, Return> {
    sequence: AtomicU64,
    call_return: CallReturnMap<Return>,
    transfer: SignalTransfer<Call, Return>,
    status_tx: tokio::sync::watch::Sender<StatusDetail>,
}

impl<Call, Return> Exchanger<Call, Return> {
    async fn send(&self, signal: Signal<Call, Return>) -> Result<(), TransferErr> {
        if let Err(_) = tokio::time::timeout(TIMEOUT, self.transfer.send(signal)).await {
            return Err(TransferErr::Timeout);
        }
        Ok(())
    }

    pub async fn call(&self, call: Call) -> Result<SigRet, TransferErr> {
        let id = self
            .sequence
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.call_return.insert(id, tx);
        let signal = Signal::call(id, call);
        self.send(signal).await?;
        match tokio::time::timeout(TIMEOUT, rx).await {
            Err(_) => Err(TransferErr::Timeout),
            Ok(Ok(ret)) => Ok(ret),
            Ok(Err(_)) => {
                /// not really sure when this condition would happen besides a flaw
                /// in the software architecture of the [ExchangeRunner]
                Err(TransferErr::Panic)
            }
        }
    }
}

struct ExchangeRunner<Call,Return> where Call: Send+Sync+'static, Return: Send+Sync+'static{
    call_return: CallReturnMap<Return>,
    status_tx: StatusTx,
    _phantom: PhantomData<Call>,
}

impl<Call,Return> ExchangeRunner<Call, Return> where Call: Send+Sync+'static, Return: Send+Sync+'static{
    pub fn new(call_return: CallReturnMap<Return>) -> Self {
        let status_tx = status_tx(StatusDetail::Ready);
        Self {
            status_tx,
            call_return,
            _phantom: PhantomData::default()
        }
    }
}



pub mod err {
    pub enum TransferErr {
        Timeout,
        Panic,
        Pending,
        Fatal,
    }
}

#[cfg(test)]
pub mod test {
    #[test]
    pub fn test() {

    }

}
