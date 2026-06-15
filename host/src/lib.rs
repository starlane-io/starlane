use std::collections::HashMap;
use std::io::Cursor;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, BufReader};
#[allow(unused)]
#[allow(warnings)]
use starlane_package::PackFile;
use starlane_package::cache::cache_singleton;

use wasmtime::component::{Component, Linker, ResourceTable};
use wasmtime::{Config, Engine, Result, Store};
use wasmtime_wasi::p2::bindings::Command;
use wasmtime_wasi::{ WasiCtx, WasiCtxView, WasiView};
use wasmtime_wasi::p2::pipe::{MemoryInputPipe, MemoryOutputPipe};

#[derive(Clone)]
pub struct HostService {
    tx: tokio::sync::mpsc::Sender<HostCmd>
}

struct HostCmd {
    pack: PackFile,
    tx: tokio::sync::oneshot::Sender<HostExecutor>
}

#[async_trait::async_trait]
pub trait Executor: Send+Sync {
    async fn run(&self, input: &str ) -> anyhow::Result<String>;
}

impl HostCmd {
    fn new(pack: PackFile) -> (Self,tokio::sync::oneshot::Receiver<HostExecutor>) {
        let (tx,rx) = tokio::sync::oneshot::channel();
            (Self {
            pack,
            tx
        },rx)
    }
}

impl HostService {
    pub fn new() -> Self {
        let tx = HostRunner::new();
        Self { tx }
    }
    pub async fn executor(&self, pack : &PackFile ) -> anyhow::Result<HostExecutor> {
        let (cmd,rx) = HostCmd::new(pack.clone());
        self.tx.send(cmd).await?;
        Ok(rx.await?)
    }
}

struct HostRunner {
    engine: Arc<Engine>,
    hosts: HashMap<PackFile,ExecHost>,
    rx: tokio::sync::mpsc::Receiver<HostCmd>
}

impl HostRunner {
    pub fn new() -> tokio::sync::mpsc::Sender<HostCmd>{

        let engine = Default::default();

        let (tx,mut rx) = tokio::sync::mpsc::channel(1);
                let runner = Self {
                    engine,
                    hosts: HashMap::default(),
                    rx,
                };
                tokio::spawn( async move {
                    runner.start().await
                });
        tx
    }

    async fn start(mut self) {
        while let Some(x) = self.rx.recv().await {
            let engine = self.engine.clone();

            let mut host = ExecHost::new(engine.as_ref(),x.pack.clone()).await.unwrap();
            let executor = host.executor();
            x.tx.send(executor);
        }
    }
}


pub struct ExecHost {
    pack: PackFile,
    component: Component,
    linker: Linker<ExecState>
}


impl ExecHost {
    pub async fn new(engine: &Engine, pack: PackFile) -> anyhow::Result<ExecHost> {
        let cache = starlane_package::cache::cache_singleton();
        let path = cache.get_path(&pack).await.unwrap();
        let mut linker: Linker<ExecState> = wasmtime::component::Linker::new(&engine);
        wasmtime_wasi::p2::add_to_linker_async(&mut linker).unwrap();
        let component = Component::from_file(&engine, &path)?;

        Ok(Self {
            component,
            linker,
            pack,
        })
    }

    fn executor(&self) -> HostExecutor {
       HostExecutor {
           pack: self.pack.clone(),
           component: self.component.clone(),
           linker: self.linker.clone()
       }
    }
}

pub struct HostExecutor {
    pack: PackFile,
    component: Component,
    linker: Linker<ExecState>
}


#[async_trait::async_trait]
impl Executor for HostExecutor{

    async fn run(&self, input: &str) -> anyhow::Result<String> {
       let mut wasi = WasiCtx::builder();

        let mut out = MemoryOutputPipe::new(1024);
        let input = input.to_string();
       wasi.stdin(MemoryInputPipe::new(input));
        wasi.stdout(out.clone());
        let ctx = wasi.build();
       let state = ExecState {
           ctx,
           table: ResourceTable::new(),
       };

       let mut store = Store::new(self.linker.engine(), state);
       let command = Command::instantiate_async(&mut store, &self.component, &self.linker).await?;
       let program_result = command.wasi_cli_run().call_run(&mut store).await?;

        let stdout_string = String::from_utf8_lossy(&out.contents()).to_string();
        let rtn = String::from_utf8(out.contents().to_vec())?;
       Ok(rtn)
   }
}



pub struct ExecState {
    pub ctx: WasiCtx,
    pub table: ResourceTable,
}

impl WasiView for ExecState {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.ctx,
            table: &mut self.table,
        }
    }
}