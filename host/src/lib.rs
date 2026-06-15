use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
#[allow(unused)]
#[allow(warnings)]
use starlane_package::PackFile;
use starlane_package::cache::cache_singleton;

use wasmtime::component::{Component, Linker, ResourceTable};
use wasmtime::{Config, Engine, Result, Store};
use wasmtime_wasi::p2::bindings::Command;
use wasmtime_wasi::{WasiCtx, WasiCtxView, WasiView};
#[derive(Clone)]
pub struct HostService {
    tx: tokio::sync::mpsc::Sender<HostCmd>
}

struct HostCmd {
    pack: PackFile,
    tx: tokio::sync::oneshot::Sender<()>
}

impl HostCmd {
    fn new(pack: PackFile) -> (Self,tokio::sync::oneshot::Receiver<()>) {
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
    pub async fn execute(&self,pack : &PackFile ) -> anyhow::Result<()> {
        let (cmd,rx) = HostCmd::new(pack.clone());
        self.tx.send(cmd).await?;
        rx.await?;
        Ok(())
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
        println!("STARTED");
        while let Some(x) = self.rx.recv().await {
            println!("EXECUTE Command!");
            let engine = self.engine.clone();

            let mut host = ExecHost::new(engine.as_ref(),x.pack.clone()).await.unwrap();
            println!("got host");
            host.run().await.unwrap();
            x.tx.send(()).unwrap();
        }
        println!("DONE");
    }
}


pub struct ExecHost {
    pack: PackFile,
    component: Component,
    store: Store<ExecState>,
    linker: Linker<ExecState>
}


impl ExecHost {
    pub async fn new(engine: &Engine, pack: PackFile) -> anyhow::Result<ExecHost>  {
println!("NEW");
        let cache = starlane_package::cache::cache_singleton();
        let path = cache.get_path(&pack).await.unwrap();
        let mut linker:Linker<ExecState> = wasmtime::component::Linker::new(&engine);
        wasmtime_wasi::p2::add_to_linker_async(&mut linker).unwrap();

        let wasi = WasiCtx::builder().inherit_stdio().inherit_args().build();
        let state = ExecState {
            ctx: wasi,
            table: ResourceTable::new(),
        };

        let store = Store::new(&engine, state);
        let component = Component::from_file(&engine, path).unwrap();
        Ok(Self {
            linker,
            pack,
            store,
            component
        })
    }
   pub fn newx(engine: &Engine, pack: PackFile) -> anyhow::Result<Self> {
       let path ={
           let pack = pack.clone();
            wasmtime_wasi::runtime::in_tokio(async move {
               let cache = starlane_package::cache::cache_singleton();
               cache.get_path(&pack).await
           })
       }?;
       let mut linker = wasmtime::component::Linker::new(&engine);
       wasmtime_wasi::p2::add_to_linker_sync(&mut linker)?;

       let wasi = WasiCtx::builder().inherit_stdio().inherit_args().build();
       let state = ExecState {
           ctx: wasi,
           table: ResourceTable::new(),
       };
       let store = Store::new(&engine, state);
       let component = Component::from_file(&engine, path)?;
       Ok(Self {
           linker,
           pack,
           store,
           component
       })
   }

   pub async fn run(&mut self) -> anyhow::Result<()> {
       println!("RUN");
       let command = Command::instantiate_async(&mut self.store, &self.component, &self.linker).await?;
       let program_result = command.wasi_cli_run().call_run(&mut self.store).await?;
       println!("SUCCESS");
       Ok(())
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