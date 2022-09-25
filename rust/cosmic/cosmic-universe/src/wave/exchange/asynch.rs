use std::sync::Arc;
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

pub struct AsyncRouter {
    pub router: Arc<dyn Router>
}

impl AsyncRouter {
    pub fn new( router: Arc<dyn Router>) -> Self {
        Self {
            router
        }
    }
}

#[async_trait]
impl Router for AsyncRouter {
    async fn route(&self, wave: UltraWave) {
        self.router.route(wave).await
    }
}