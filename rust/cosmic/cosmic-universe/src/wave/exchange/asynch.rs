use crate::wave::exchange::{BroadTxRouter, TxRouter};
use crate::wave::UltraWave;

#[async_trait]
impl Router for TxRouter {
    async fn route(&self, wave: UltraWave) {
        self.tx.send(wave).await;
    }
}

#[async_trait]
impl Router for BroadTxRouter {
    async fn route(&self, wave: UltraWave) {
        self.tx.send(wave);
    }
}

#[async_trait]
pub trait Router: Send + Sync {
    async fn route(&self, wave: UltraWave);
}
