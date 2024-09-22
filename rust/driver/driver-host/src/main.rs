use futures::FutureExt;
use futures_core::future::BoxFuture;
use std::any::Any;
use std::ffi::OsStr;
use std::fmt::{Debug, Formatter};
use std::fs;
use std::io::{Error, Read, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::io;
use tokio::io::{AsyncRead, AsyncSeek, AsyncWrite, ReadBuf};
use tokio::runtime::Handle;
use virtual_fs::{
    DeviceFile, FileOpener, FileSystem, FsError, Metadata, OpenOptions, OpenOptionsConfig,
    PassthruFileSystem, ReadDir, RootFileSystemBuilder, Upcastable, VirtualFile,
};

use wasmer::{Module, Store};
use wasmer_wasix::{types::__WASI_STDIN_FILENO, Pipe, WasiEnv, WasiEnvBuilder, WasiTtyState};

fn main() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    println!(
        "PWD: {}",
        std::env::current_dir().unwrap().as_path().display()
    );

    let (stdout_tx, mut stdout_rx) = Pipe::channel();
    let (stderr_tx, mut stderr_rx) = Pipe::channel();
    let (mut stdin_sender, stdin_reader) = Pipe::channel();

    let write = "Hello All you People";
    write!(stdin_sender, "{}", write);
    stdin_sender.close();

    RootFileSystemBuilder::new();
    /*    let root_fs = RootFileSystemBuilder::new().with_tmp(false).with_stdin(Box::new(stdin_reader)).with_stdout(Box::new(stdout_tx)).with_stderr(Box::new(stderr_tx))
           .default_root_dirs(false).build();

       let root_fs = RootFileSystemBuilder::new()
           .with_tmp(true)// Optional: Add tmp folder
           .build();

    */

    /*
    let mut dev_null = root_fs.mount()
        .new_open_options()
        .read(true)
        .write(true)
        .open("/dev/null")
        .unwrap();

     */
    // Create a task manager using Tokio

    runtime.block_on(async move {

               let mut fs = virtual_fs::host_fs::FileSystem::new(Handle::current(), ".").unwrap();
        //        let mut fs = virtual_fs::host_fs::FileSystem::new(Handle::current(), ".").unwrap();
   //     let mut fs = HostFileSystem::new( "scratch".into());
        /*        fs.open( &".".into(), & virtual_fs::OpenOptionsConfig {
                   read: true,
                   write: true,
                   create_new: true,
                   create: true,
                   append: true,
                   truncate: true,
               });

        */
        //    let root_fs = Box::new(root_fs) as Box<dyn virtual_fs::VirtualFile + Send + Sync + 'static>

        let mut builder = WasiEnv::builder("filestore")
            .args(&["test"])
            //.args(&["write", "file.out"])
            //            .args(&["list"])
            .preopen_dir(".").unwrap()
            .fs(Box::new(fs));

        let wasm_path = "wasix.wasm";
        // Let's declare the Wasm module with the text representation.
        let wasm_bytes = std::fs::read(wasm_path).unwrap();

        // Create a Store.
        let mut store = Store::default();

        println!("Compiling module...");
        // Let's compile the Wasm module.
        let module = Module::new(&store, wasm_bytes).unwrap();

        builder.run_with_store(module, &mut store).unwrap();
        println!("done");
    });

    //    WasiEnvBuilder::preopen_vfs_dirs( & mut builder, vec![String::from(format!("{}/scratch/", std::env::current_dir().unwrap().as_path().display()))]);
    // .env("KEY", "Value")

    eprintln!("Run complete - reading output");
    stdin_sender.close();
    stdout_rx.close();
    stderr_rx.close();

    //    let mut buf = String::new();
    //    stdout_rx.read_to_string(&mut buf).unwrap();

    //   eprintln!("Output: {buf}");
}




#[cfg(test)]
mod test {
    use tokio::runtime::Handle;
    use virtual_fs::{host_fs, FileSystem};

    #[tokio::test]
    pub async fn test_host_fs() {
//        let mut fs = virtual_fs::host_fs::FileSystem::new(Handle::current(), ".").unwrap();
//        fs.create_dir(Path::new("test")).unwrap()
//        fs.mount().unwrap()
    }

}