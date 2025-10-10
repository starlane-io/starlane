use reqwest;
use std::path::PathBuf;
use anyhow::Result;
use crate::create::{PackErr, PackageDirectoryStructure};
use crate::PackageErr;

pub struct PackageRepo {
    pub url: String
}

impl PackageRepo {
    pub fn new( url:String) -> Self {
        Self { url }
    }
}

impl Default for PackageRepo {
    fn default() -> Self {
        Self {
            url: "localhost:3000".to_string()
        }
    }
}

impl PackageRepo {


    /// Upload a zip file to the package-server
    pub async fn upload(&self, pds: &PackageDirectoryStructure) -> Result<(),PackageErr> {
println!("uploading zip file");

        let tmp_file= pds.zip()?;
        let zip_path= tmp_file.path().to_path_buf();
println!("zip_path: {}", zip_path.display());
        // Read the zip file
        let file_bytes = tokio::fs::read(&zip_path).await?;

        println!("Uploading {} bytes", file_bytes.len());
        // Get the filename
        let file_name = zip_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("archive");

        println!("file_name {}...", file_name);
        // Create multipart form
        let form = reqwest::multipart::Form::new()
            .part(
                "file",
                reqwest::multipart::Part::bytes(file_bytes)
                    .file_name("file")
                    .mime_str("application/zip")?,
            );

        println!("form: {:?}", form);
        // Send the POST request
        let client = reqwest::Client::new();
println!("url: {}/zip",self.url);
        let response = client
            .post(format!("http://{}/zip", self.url))
            .multipart(form)
            .send()
            .await?;

println!("response: {:?}", response);
        if response.status().is_success() {
            let body = response.text().await?;
            println!("Upload successful: {}", body);
            Ok(())
        } else {
            let status = response.status();
            let error_text = response.text().await?;
            Err(PackageErr::UploadErr(format!("Upload failed with status {}: {}", status, error_text).to_string()))
        }
    }

    /// Download a zip file from the package-server
    async fn download_zip_file(server_url: &str, file_id: &str, output_path: PathBuf) -> Result<()> {
        let client = reqwest::Client::new();
        let response = client
            .get(format!("{}/zip/{}", server_url, file_id))
            .send()
            .await?;

        if response.status().is_success() {
            let bytes = response.bytes().await?;
            tokio::fs::write(&output_path, bytes).await?;
            println!("Download successful: saved to {:?}", output_path);
            Ok(())
        } else {
            let status = response.status();
            let error_text = response.text().await?;
            anyhow::bail!("Download failed with status {}: {}", status, error_text);
        }
    }
}

