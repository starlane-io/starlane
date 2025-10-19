use crate::create::{PackErr, PackageLayout};
use crate::repo::SourceRepo;
use crate::zip::{unzip_from_binary_to_temp, ZipError};
use crate::IgnoreObserver;
use axum::extract::multipart::Multipart;
use axum::extract::State;
use axum::routing::method_routing::{get, post};
use axum::routing::Router;
use axum_core::response::{IntoResponse, Response};
use reqwest::{header, StatusCode};
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::fs;
use tokio::io::AsyncReadExt;

pub struct ServerBuilder {
    pub bind: String,
    pub repo: SourceRepo
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

    pub fn temp() -> (Self,TempDir) {
        let (repo,dir)= SourceRepo::temp();
        (Self {
            bind: "0.0.0.0:3000".to_string(),
            repo
        },dir)
    }
    pub fn router(&self) -> Router
    {
        let state = Arc::new(RepoState::new(self.repo.clone()));
        let router = Router::new()
            .route("/zip", post(upload_zip))
            .with_state(state)
            .route("/zip/{id}", get(download_zip));

        router
    }

    pub fn serve(self) -> tokio::sync::oneshot::Sender<()> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        tokio::spawn( async move {
            println!("Starting server....");
            let router = self.router();
            // Run the server
            let listener = tokio::net::TcpListener::bind(self.bind.clone())
                .await
                .expect("Failed to bind to address");

            println!("Server running on http://{}", self.bind);
            println!("POST /zip - Upload a zip file");
            println!("GET /zip/:id - Download a zip file");
            async fn stop(rx: tokio::sync::oneshot::Receiver<()>) {
                rx.await.unwrap()
            }

            axum::serve(listener, router)
                .with_graceful_shutdown(stop(rx))
                .await
                .expect("Failed to start server");

            println!("Server stopped");
        });
        tx
    }


}


pub struct RepoState {
    pub repo: SourceRepo,
}

impl RepoState {
    pub fn new( repo: SourceRepo ) -> Self {
        Self {
            repo
        }
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
    println!("Received Ctrl+C, initiating graceful shutdown...");
}
/// Handler for uploading zip files
async fn upload_zip(
    app: State<Arc<RepoState>>,
    mut multipart: Multipart,
) -> Result<Response, AppError> {
    println!(".... uploading zip file");
    let storage_dir = PathBuf::from("./zip_storage");

    while let Some(field) = multipart.next_field().await? {
        let name = field.name().unwrap_or("").to_string();
        println!(". field name: {}", name);
        let file_name = field.file_name().unwrap_or("").to_string();

        // Generate unique ID for the file

        // Read the file data
        let data = field.bytes().await?;
        println!(". data.len: {}", data.len());
        let dir = unzip_from_binary_to_temp(data.as_ref())?;
        println!(". dir: {:?}", dir);

        let mut observer = IgnoreObserver;
        let path = dir.path().to_path_buf();
        let layout = PackageLayout::create(&path, &mut observer)?;

        layout.diagnose();

        app.repo.save_package(layout)?;

        println!("\n\npackage saved...\n\n");
        // Return the file ID to the client
        return Ok((StatusCode::CREATED, format!("File uploaded successfully.")).into_response());
    }

    Err(AppError::NoFileProvided)
}

/// Handler for downloading zip files
async fn download_zip(
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Response, AppError> {
    let storage_dir = PathBuf::from("./zip_storage");
    let file_path = storage_dir.join(format!("{}.zip", id));

    // Check if file exists
    if !file_path.exists() {
        return Err(AppError::FileNotFound);
    }

    // Read the file
    let mut file = fs::File::open(&file_path).await?;
    let mut contents = Vec::new();
    file.read_to_end(&mut contents).await?;

    println!("Downloading zip file with ID: {}", id);

    // Return the file as a response
    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/zip"),
            (
                header::CONTENT_DISPOSITION,
                &format!("attachment; filename=\"{}.zip\"", id),
            ),
        ],
        contents,
    )
        .into_response())
}

/// Custom error type for the application
#[derive(Debug)]
enum AppError {
    NoFileProvided,
    InvalidFileType,
    FileNotFound,
    #[allow(dead_code)]
    IoError(std::io::Error),
    #[allow(dead_code)]
    MultipartError(axum::extract::multipart::MultipartError),
    #[allow(dead_code)]
    ZipError(ZipError),
    #[allow(dead_code)]
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
            AppError::NoFileProvided => (StatusCode::BAD_REQUEST, "No file provided"),
            AppError::InvalidFileType => (StatusCode::BAD_REQUEST, "Only .zip files are allowed"),
            AppError::FileNotFound => (StatusCode::NOT_FOUND, "File not found"),
            AppError::IoError(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error"),
            AppError::MultipartError(_) => {
                (StatusCode::BAD_REQUEST, "Failed to process multipart data")
            }
            AppError::ZipError(_) => (StatusCode::BAD_REQUEST, "Failed to process zip file"),
            AppError::PackErr(err) => (StatusCode::BAD_REQUEST, "could not process package zip"),
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

#[cfg(test)]
pub mod test {
    use axum_test::TestServer;
    use crate::server::ServerBuilder;

    #[test]
    pub fn test() {
        // must hang on to TempDir ref until test is finished
        let (server,tmp) = ServerBuilder::temp();
        let server = TestServer::new( server.router()).unwrap();
        //server.post("/zip").await.();
        todo!()
    }
}