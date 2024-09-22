use tokio::sync::watch::error::SendError;
use crate::particle::Status;


pub fn state_relay<S>( initial: S ) -> (tokio::sync::mpsc::Sender<S>,tokio::sync::watch::Receiver<S>) where S: Clone+Send+Sync+'static
{
    let (mpsc_tx,mut mpsc_rx) = tokio::sync::mpsc::channel(8);
    let (watch_tx, watch_rx) = tokio::sync::watch::channel(initial);

    tokio::task::spawn( async move {
        while let Some(state) = mpsc_rx.recv().await {
            match watch_tx.send(state) {
                Err(err) => {}
                _ => {}
            }
        }
    });

    (mpsc_tx,watch_rx)
}