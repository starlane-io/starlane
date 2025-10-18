use reqwest;
use std::path::PathBuf;
use anyhow::Result;
use async_trait::async_trait;
use starlane_space::types::specific::Specific;
use crate::create::{PackErr, PackageLayout};
use crate::{PackObserver, PackageErr, PublishObserver};
use crate::repo::Repo;

pub struct RemoteRepo {
    pub url: String
}

#[async_trait]
impl Repo for RemoteRepo {
    async fn get_slice(&self, specific: &Specific) -> std::result::Result<Vec<u8>, PackageErr> {
        todo!()
    }

    /// Upload a zip file to the package-server
    async fn submit<P>(&self, pds: & PackageLayout, observer:  P) -> anyhow::Result<(), PackageErr> where P: PublishObserver+Send+Sync {
        observer.start_upload(&self.url);

        let tmp_file= pds.zip()?;
        let zip_path= tmp_file.path().to_path_buf();
        // Read the zip file
        let file_bytes = tokio::fs::read(&zip_path).await?;

        // Get the filename
        let file_name = zip_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("archive");

        // Create multipart form
        let form = reqwest::multipart::Form::new()
            .part(
                "file",
                reqwest::multipart::Part::bytes(file_bytes)
                    .file_name("file")
                    .mime_str("application/zip")?,
            );

        // Send the POST request
        let client = reqwest::Client::new();
        let response = client
            .post(format!("http://{}/zip", self.url))
            .multipart(form)
            .send()
            .await?;

        if response.status().is_success() {
            let body = response.text().await?;
            Ok(())
        } else {
            let status = response.status();
            let error_text = response.text().await?;
            Err(PackageErr::UploadErr(format!("Upload failed with status {}: {}", status, error_text).to_string()))
        }
    }
}

impl RemoteRepo {
    pub fn new( url:String) -> Self {
        Self { url }
    }


    /*
    /// publish the current directory
    pub async fn publish( &self, observer: &mut dyn PublishObserver )  -> Result<(),PackageErr>{
        let server = PackageRepo::default();
        let path =  std::env::current_dir().unwrap();
        let pds = PackageLayout::create(&path, observer).unwrap();
        server.upload(&pds, observer).await
    }
    
     */
}


impl Default for RemoteRepo {
    fn default() -> Self {
        Self {
            url: "localhost:3000".to_string()
        }
    }
}

impl RemoteRepo {




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
            Ok(())
        } else {
            let status = response.status();
            let error_text = response.text().await?;
            anyhow::bail!("Download failed with status {}: {}", status, error_text);
        }
    }
}

