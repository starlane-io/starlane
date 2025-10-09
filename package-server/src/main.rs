
use axum::{
    body::Body,
    extract::Multipart,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use axum::extract::State;
use tokio::fs;
use tokio::io::AsyncReadExt;
use uuid::Uuid;

#[tokio::main]
async fn main() {
    start(RepoApp::default()).await;
}

pub struct RepoApp {
    pub bind: String
}

impl Default for RepoApp {
    fn default() -> Self {
        Self {
            bind: "0.0.0.0:3000".to_string()
        }
    }
}


pub async fn start(repo: RepoApp) {
    let repo = Arc::new(repo);
    // Build the router
    let app = Router::new()
        .route("/zip", post(upload_zip)).with_state(repo.clone())
        .route("/zip/{id}", get(download_zip));

    // Run the server
    let listener = tokio::net::TcpListener::bind(repo.bind.clone())
        .await
        .expect("Failed to bind to address");

    println!("Server running on http://0.0.0.0:3000");
    println!("POST /zip - Upload a zip file");
    println!("GET /zip/:id - Download a zip file");

    axum::serve(listener, app)
        .await
        .expect("Failed to start server");
}

/// Handler for uploading zip files
async fn upload_zip(app: State<Arc<RepoApp>>, mut multipart: Multipart) -> Result<Response, AppError> {
    let storage_dir = PathBuf::from("./zip_storage");

    while let Some(field) = multipart.next_field().await? {
        let name = field.name().unwrap_or("").to_string();
        let file_name = field.file_name().unwrap_or("").to_string();

        // Only process zip files
        if !file_name.ends_with(".zip") {
            return Err(AppError::InvalidFileType);
        }

        // Generate unique ID for the file
        let file_id = Uuid::new_v4();
        let file_path = storage_dir.join(format!("{}.zip", file_id));

        // Read the file data
        let data = field.bytes().await?;

        // Save the file
        let mut file = std::fs::File::create(&file_path)?;
        file.write_all(&data)?;

        println!("Uploaded zip file: {} (ID: {})", file_name, file_id);

        // Return the file ID to the client
        return Ok((
            StatusCode::CREATED,
            format!("File uploaded successfully. ID: {}", file_id),
        )
            .into_response());
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



mod test {
    use std::path::PathBuf;
    use starlane_package::server::PackageRepo;

    #[tokio::test]
    async fn test() -> anyhow::Result<()> {
        let server = PackageRepo::default();

        /*
        // Example: Upload a zip file
        let zip_to_upload = PathBuf::from("./my-archive.zip");

        println!("Uploading zip file: {:?}", zip_to_upload);
        let response = upload_zip_file(server_url, zip_to_upload).await?;

        // Extract the UUID from the response
        // Response format: "File uploaded successfully. ID: <uuid>"
        if let Some(id) = response.split("ID: ").nth(1) {
            let file_id = id.trim();
            println!("File ID: {}", file_id);

            // Example: Download the same file
            let download_path = PathBuf::from("./downloaded-archive.zip");
            println!("Downloading file with ID: {}", file_id);
            download_zip_file(server_url, file_id, download_path).await?;
        }

         */

        Ok(())
    }
}