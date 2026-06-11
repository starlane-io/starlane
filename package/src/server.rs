use crate::create::PackageLayout;
use crate::repo::{Repo, SourceRepo};
use crate::zip::{unzip_from_binary_to_temp, ZipError};
use crate::{new_ignorant_observer, IgnoreObserver, PackObserver, PackageErr};
use axum::extract::multipart::Multipart;
use axum::extract::{Query, State};
use axum::routing::method_routing::{get, post};
use axum::routing::Router;
use axum_core::response::{IntoResponse, Response};
use port_check::free_local_port;
use reqwest::{header, StatusCode};
use serde_derive::Deserialize;
use starlane_space::err::ParseErrs0;
use starlane_space::types::specific::Slice;
use std::io;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::net::TcpListener;
use tokio::sync::{oneshot, watch};

use crate::remote::DEFAULT_PORT;

pub struct ServerBuilder {
    pub bind: ServerBind,
    pub repo: SourceRepo,
}

impl Default for ServerBuilder {
    fn default() -> Self {
        Self {
            bind: ServerBind::default(),
            repo: SourceRepo::default(),
        }
    }
}

impl ServerBuilder {
    pub fn temp() -> Self {
        let repo = SourceRepo::temp();
        Self {
            bind: ServerBind::RandomAvailable,
            repo,
        }
    }

    #[cfg(test)]
    pub async fn mock() -> ServerControl {
        let repo = SourceRepo::mock().await;
        let server = Self {
            bind: ServerBind::RandomAvailable,
            repo,
        };

        let control = server.start();

        control
    }

    fn router(&self) -> Router {
        let state = Arc::new(RepoState::new(self.repo.clone()));
        let router = Router::new()
            .route("/package", post(upload_zip))
            .with_state(state.clone())
            .route("/slice", get(get_slice))
            .with_state(state);
        router
    }

    /// start a server that will run until the process is terminated
    pub async fn start_no_controller(self) {
        let handle = self.start();
        loop {
            tokio::time::sleep(Duration::from_secs(u64::MAX)).await;
        }
    }

    pub fn start(self) -> ServerControl {
        let (port_tx, port_rx) = oneshot::channel();
        let (mut signals, control) = ServerSignals::new();
        async fn run(
            this: &ServerBuilder,
            request_terminate: oneshot::Receiver<()>,
            status: watch::Sender<ServerStatus>,
            port_tx: oneshot::Sender<u16>,
        ) -> Result<(), ServerErr> {
            let router = this.router();
            // Run the server
            let (listener, port) = this.bind.create().await?;

            port_tx.send(port);

            axum::serve(listener, router)
                .with_graceful_shutdown(stop(request_terminate))
                .await?;
            Ok(())
        }

        async fn stop(rx: tokio::sync::oneshot::Receiver<()>) {
            rx.await.unwrap();
        }

        let request_terminate = signals.request_terminate.take().unwrap();

        signals.status.send(ServerStatus::Pending).unwrap();
        let mut status = signals.status.clone();

        tokio::spawn(async move {
            if let Err(err) = run(&self, request_terminate, status.clone(), port_tx).await {
                status.send(err.into()).unwrap();
            } else {
                status.send(ServerStatus::Terminated).unwrap();
            }
        });

        /// this block of code will set the status to [ServerStatus::Ready] as soon as the server health is confirmed.
        /// I found this necessary in order to make the testing framework pass because sometimes servers take a while
        /// to get started...
        tokio::spawn(async move {
            let port = match tokio::time::timeout(Duration::from_secs(30), port_rx).await {
                Ok(Ok(port)) => port,
                _ => {
                    signals
                        .status
                        .send(ServerStatus::panic(ServerErr::HealthcheckTimeout))
                        .unwrap_or_default();
                    return;
                }
            };

            let client = reqwest::Client::builder().build().unwrap();
            const interval: u64 = 250u64;
            const retry_seconds: u64 = 100;

            let mut count = retry_seconds / interval;
            loop {
                if let Ok(response) = client
                    .get(format!("http://localhost:{}/status", port))
                    .send()
                    .await
                {
                    signals.status.send(ServerStatus::ready(port)).unwrap();
                } else {
                    tokio::time::sleep(Duration::from_millis(250u64)).await;
                    count = count - 1;
                }
                if count < 0 {
                    signals
                        .status
                        .send(ServerStatus::Panic(ServerErr::HealthcheckTimeout.into()));
                }
            }
        });

        control
    }
}

struct ServerSignals {
    /// receiver tracking the [ServerControl]'s desire to keep the server running
    request_terminate: Option<oneshot::Receiver<()>>,
    /// will send or drop after the server has stopped
    terminated: oneshot::Sender<()>,
    /// report the present [ServerStatus]
    status: watch::Sender<ServerStatus>,
}

impl ServerSignals {
    fn new() -> (Self, ServerControl) {
        let (request_terminate_tx, request_terminate_rx) = oneshot::channel();
        let (alive_tx, alive_rx) = oneshot::channel();
        let (status_tx, status_rx) = watch::channel(ServerStatus::Unknown);
        (
            Self {
                request_terminate: Some(request_terminate_rx),
                terminated: alive_tx,
                status: status_tx,
            },
            ServerControl {
                request_terminate: request_terminate_tx,
                terminated: alive_rx,
                status: status_rx,
            },
        )
    }
}

enum ServerBind {
    Port(u16),
    RandomAvailable,
}

impl ServerBind {
    pub(crate) async fn create(&self) -> Result<(TcpListener, u16), ServerErr> {
        match self {
            ServerBind::Port(port) => TcpListener::bind(format!("0.0.0.0:{}", port))
                .await
                .map_err(|e| e.into())
                .map(|listener| (listener, port.clone())),
            ServerBind::RandomAvailable => {
                let port = free_local_port().ok_or(ServerErr::FreePortNotFound)?;
                TcpListener::bind(format!("0.0.0.0:{}", port))
                    .await
                    .map_err(|e| e.into())
                    .map(|listener| (listener, port))
            }
        }
    }
}

impl Default for ServerBind {
    fn default() -> Self {
        Self::Port(DEFAULT_PORT)
    }
}

#[derive(Error, Debug)]
pub enum ServerErr {
    #[error("{0}")]
    Io(#[from] io::Error),
    #[error("port_check::free_local_port(): could not find a free port")]
    FreePortNotFound,
    #[error("HealthcheckTimeout")]
    HealthcheckTimeout,
}

#[derive(Clone, Debug)]
pub enum ServerStatus {
    Unknown,
    Pending,
    Ready { port: u16 },
    Terminated,
    Panic(Arc<ServerErr>),
}

impl ServerStatus {
    pub fn ready(port: u16) -> Self {
        Self::Ready { port }
    }

    pub fn panic(err: ServerErr) -> Self {
        Self::Panic(Arc::new(err))
    }
}

impl From<ServerErr> for ServerStatus {
    fn from(err: ServerErr) -> Self {
        ServerStatus::Panic(err.into())
    }
}

pub struct ServerControl {
    /// when dropped signals the server to stop
    request_terminate: oneshot::Sender<()>,
    /// completes when the server has stopped from a termination request
    terminated: oneshot::Receiver<()>,
    /// display the current status of the server
    status: watch::Receiver<ServerStatus>,
}

impl ServerControl {
    pub async fn stop(mut self) {
        self.request_terminate.send(()).unwrap();
        self.terminated.await.unwrap_or_default();
    }

    pub async fn await_termination(self) {
        self.terminated.await.unwrap_or_default();
    }

    pub async fn await_ready(&self) -> ServerStatus {
        let mut status = self.status.clone();
        while let Ok(_) = status.changed().await {
            let status = self.status.borrow().clone();
            match &status {
                /// this is what we want!
                ServerStatus::Ready { .. } => {
                    return status;
                }
                ServerStatus::Terminated => {
                    return ServerStatus::panic(ServerErr::HealthcheckTimeout.into());
                }
                ServerStatus::Panic(panic) => {
                    return status;
                }
                /// in all other cases keep waiting...
                _ => {}
            }
        }

        ServerStatus::panic(ServerErr::HealthcheckTimeout.into())
    }

    pub async fn get_port(&self) -> Result<u16, ServerStatus> {
        let status = self.await_ready().await;
        match status {
            ServerStatus::Ready { port } => Ok(port),
            status => Err(status),
        }
    }
}

#[derive(Debug, Deserialize)]
struct SliceParams {
    slice: String,
}
pub struct RepoState {
    pub repo: SourceRepo,
}

impl RepoState {
    pub fn new(repo: SourceRepo) -> Self {
        Self { repo }
    }
}

impl Default for RepoState {
    fn default() -> Self {
        Self {
            repo: SourceRepo::default(),
        }
    }
}

/*
pub async fn start_package_server() {
    let repo = RepoState::default();
    let repo = Arc::new(repo);
    // Build the router
    let app = Router::new()
        .route("/zip", post(upload_zip))
        .with_state(repo.clone())
        .route("/zip/{id}", get(download_zip));

    // Run the server
    let listener = tokio::net::TcpListener::bind(repo.bind.clone())
        .await
        .expect("Failed to bind to address");

    println!("Server running on http://0.0.0.0:3000");
    println!("POST /zip - Upload a zip file");
    println!("GET /zip/:id - Download a zip file");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("Failed to start server");
}

 */

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to listen for Ctrl+C signal");
}
/// Handler for uploading zip files
async fn upload_zip(
    state: State<Arc<RepoState>>,
    mut multipart: Multipart,
) -> Result<Response, AppError> {
    let storage_dir = PathBuf::from("./zip_storage");

    while let Some(field) = multipart.next_field().await? {
        let name = field.name().unwrap_or("").to_string();
        let file_name = field.file_name().unwrap_or("").to_string();

        // Generate unique ID for the file

        // Read the file data
        let data = field.bytes().await?;
        let dir = unzip_from_binary_to_temp(data.as_ref())?;

        let mut observer: Box<dyn PackObserver> = new_ignorant_observer();
        let path = dir.path().to_path_buf();
        let layout = PackageLayout::create(&path, &*observer).map_err(Into::<PackageErr>::into)?;

        layout.diagnose();

        state.repo.publish(&layout).await?;

        // Return the file ID to the client
        return Ok((StatusCode::CREATED, format!("File uploaded successfully.")).into_response());
    }

    Err(AppError::NoFileProvided)
}

/// Handler for downloading zip files
async fn get_slice(
    state: State<Arc<RepoState>>,
    params: Query<SliceParams>,
) -> Result<Response, AppError> {
    let slice = Slice::from_str(params.slice.as_str())?;

    let contents = state.repo.get_slice(&slice).await?;

    // Return the file as a response
    Ok((
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/zip")],
        contents,
    )
        .into_response())
}

/// Custom error type for the application
#[derive(Debug, Error)]
enum AppError {
    #[error("No file provided. Please provide a package file to upload.")]
    NoFileProvided,
    #[error("Invalid file type. Only .zip files are allowed.")]
    InvalidFileType,
    #[allow(dead_code)]
    #[error("Illegal Slice Name: '{0}'")]
    IllegalSliceName(#[from] ParseErrs0),
    #[error("File not found.")]
    FileNotFound,
    #[allow(dead_code)]
    #[error("{0}")]
    IoError(std::io::Error),
    #[allow(dead_code)]
    #[error("{0}")]
    MultipartError(axum::extract::multipart::MultipartError),
    #[allow(dead_code)]
    #[error("{0}")]
    ZipError(ZipError),
    #[allow(dead_code)]
    #[error("{0}")]
    PackErr(#[from] PackageErr),
    #[allow(dead_code)]
    #[error("{0}")]
    ServerErr(#[from] ServerErr),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::NoFileProvided => (StatusCode::BAD_REQUEST, "No file provided".to_string()),
            AppError::InvalidFileType => (
                StatusCode::BAD_REQUEST,
                "Only .zip files are allowed".to_string(),
            ),
            AppError::FileNotFound => (StatusCode::NOT_FOUND, "File not found".to_string()),
            AppError::IoError(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
            ),
            AppError::MultipartError(_) => (
                StatusCode::BAD_REQUEST,
                "Failed to process multipart data".to_string(),
            ),
            AppError::ZipError(_) => (
                StatusCode::BAD_REQUEST,
                "Failed to process zip file".to_string(),
            ),
            AppError::PackErr(err) => (
                StatusCode::BAD_REQUEST,
                "could not process package zip".to_string(),
            ),
            AppError::IllegalSliceName(err) => (
                StatusCode::BAD_REQUEST,
                format!("Illegal Slice Name: '{}'", err),
            ),
            AppError::ServerErr(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Internal Server Error: '{}'", err),
            ),
        };

        (status, message).into_response()
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::IoError(err)
    }
}

impl From<axum::extract::multipart::MultipartError> for AppError {
    fn from(err: axum::extract::multipart::MultipartError) -> Self {
        AppError::MultipartError(err)
    }
}

impl From<ZipError> for AppError {
    fn from(err: ZipError) -> Self {
        AppError::ZipError(err)
    }
}
