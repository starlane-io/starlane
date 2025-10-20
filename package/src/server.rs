use crate::create::{PackErr, PackageLayout};
use crate::repo::SourceRepo;
use crate::zip::{unzip_from_binary_to_temp, ZipError};
use crate::IgnoreObserver;
use axum::extract::multipart::Multipart;
use axum::extract::{Query, State};
use axum::routing::method_routing::{get, post};
use axum::routing::Router;
use axum_core::response::{IntoResponse, Response};
use reqwest::{header, StatusCode};
use serde_derive::Deserialize;
use starlane_space::err::ParseErrs0;
use starlane_space::types::specific::Slice;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use tempfile::TempDir;
use thiserror::Error;

pub struct ServerBuilder {
    pub bind: String,
    pub repo: SourceRepo,
}

impl Default for ServerBuilder {
    fn default() -> Self {
        Self {
            bind: "0.0.0.0:3000".to_string(),
            repo: SourceRepo::default(),
        }
    }
}

impl ServerBuilder {
    pub fn temp() -> (Self, TempDir) {
        let (repo, dir) = SourceRepo::temp();
        (
            Self {
                bind: "0.0.0.0:3000".to_string(),
                repo,
            },
            dir,
        )
    }


    #[cfg(test)]
    pub fn mock() -> tokio::sync::oneshot::Sender<()> {
        let (repo, dir) = SourceRepo::mock();
        let server = Self {
            bind: "0.0.0.0:3000".to_string(),
            repo,
        };


        let handle = server.serve();
        let (tx, rx) = tokio::sync::oneshot::channel();

        tokio::spawn(async move {
           rx.await.unwrap();
           drop(handle);
           drop(dir);
        });

        tx
    }

    pub fn router(&self) -> Router {
        let state = Arc::new(RepoState::new(self.repo.clone()));
        let router = Router::new()
            .route("/package", post(upload_zip))
            .with_state(state.clone())
            .route("/slice", get(get_slice))
            .with_state(state);
        router
    }

    pub fn serve(self) -> tokio::sync::oneshot::Sender<()> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            let router = self.router();
            // Run the server
            let listener = tokio::net::TcpListener::bind(self.bind.clone())
                .await
                .expect("Failed to bind to address");

            async fn stop(rx: tokio::sync::oneshot::Receiver<()>) {
                rx.await.unwrap()
            }

            axum::serve(listener, router)
                .with_graceful_shutdown(stop(rx))
                .await
                .expect("Failed to start server");
        });
        tx
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

        let mut observer = IgnoreObserver;
        let path = dir.path().to_path_buf();
        let layout = PackageLayout::create(&path, &mut observer)?;

        layout.diagnose();

        state.repo.submit(layout)?;

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
    PackErr(PackErr),
}

impl From<PackErr> for AppError {
    fn from(err: PackErr) -> Self {
        AppError::PackErr(err)
    }
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
