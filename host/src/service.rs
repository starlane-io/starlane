use std::sync::Arc;
use wasmtime::Engine;

pub struct HostService {
    engine: Arc<Engine>,
}

