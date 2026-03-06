use crate::create::PackageLayout;
pub(crate) use crate::repo::Repo;
use crate::repo::RepoStatus;
use crate::{PackageErr, PublishObserver};
use anyhow::Result;
use async_trait::async_trait;
use reqwest;
use reqwest::Client;
use starlane_space::types::specific::Slice;
use std::path::PathBuf;

pub const DEFAULT_PORT: u16 = 3000u16;

#[derive(Clone)]
pub struct RemoteRepo {
    pub url: String,
    pub client: Client,
}

#[async_trait]
impl Repo for RemoteRepo {

    async fn get_slice(&self, slice: &Slice) -> std::result::Result<Vec<u8>, PackageErr> {
        Ok(self
            .client
            .get(format!("http://{}/slice", self.url))
            .query(&[("slice", slice.to_string())])
            .send()
            .await
            .unwrap()
            .bytes()
            .await?
            .into())
    }

    async fn publish_with_listener(
        &self,
        layout: &PackageLayout,
        listener: &Box<dyn PublishObserver>,
    ) -> Result<(), PackageErr> {
    /// Upload a zip file to the package-server

        listener.start_upload(&self.url);

        let tmp_file = layout.zip()?;
        let zip_path = tmp_file.path().to_path_buf();
        // Read the zip file
        let file_bytes = tokio::fs::read(&zip_path).await?;

        // Get the filename
        let file_name = zip_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("archive");

        // Create multipart form
        let form = reqwest::multipart::Form::new().part(
            "file",
            reqwest::multipart::Part::bytes(file_bytes)
                .file_name("file")
                .mime_str("application/zip")?,
        );

        // Send the POST request
        let response = self.client
            .post(format!("http://{}/package", self.url))
            .multipart(form)
            .send()
            .await?;

        if response.status().is_success() {
            let body = response.text().await?;
            Ok(())
        } else {
            let status = response.status();
            let error_text = response.text().await?;
            Err(PackageErr::UploadErr(
                format!("Upload failed with status {}: {}", status, error_text).to_string(),
            ))
        }
    }

    async fn status(&self) -> RepoStatus {
        match self.client
            .get(format!("http://{}/status", self.url)).send().await {
            Ok(response) => {
                response.status().into()
            }
            Err(err) => {
                err.into()
            }
        }
    }
}

impl RemoteRepo {
    pub fn new(url: String) -> Self {
        let client = reqwest::Client::new();
        Self { url, client }
    }

    /// Download a zip file from the package-server
    async fn download_zip_file(
        server_url: &str,
        file_id: &str,
        output_path: PathBuf,
    ) -> Result<()> {
        let client = reqwest::Client::new();
        let response = client
            .get(format!("{}/zip/{}", server_url, file_id))
            .send()
            .await?;

        if response.status().is_success() {
            let bytes = response.bytes().await?;
            tokio::fs::write(&output_path, bytes).await?;
            Ok(())
        } else {
            let status = response.status();
            let error_text = response.text().await?;
            anyhow::bail!("Download failed with status {}: {}", status, error_text);
        }
    }

    pub fn local_with_port(port: u16) -> Self {
        let client = reqwest::Client::new();
        Self {
            url: format!("localhost:{}",port).to_string(),
            client,
        }
    }
}

impl Default for RemoteRepo {
    fn default() -> Self {
        Self::local_with_port(DEFAULT_PORT)
    }
}
