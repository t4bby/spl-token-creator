use std::fs::File;
use serde_json::Value;
use solana_client::client_error::reqwest;
use thiserror::Error;

pub const API_URL: &str = "https://api.nft.storage";

#[derive(Debug, Error)]
pub enum UploadError {
    #[error("failed on client: {0}")]
    ClientError(String),

    #[error("upload request error: {0}")]
    UploadRequestError(String),

    #[error("parse error: {0}")]
    ParseError(String),
}


#[derive(Debug, Error)]
pub enum MetadataError {
    #[error("failed to generate metadata: {0}")]
    GenerateError(String),
}

pub fn generate_metadata(project_dir: &str, name: &str, symbol: &str, description: &str, image: &str) -> Result<(), MetadataError> {
    let metadata = format!(
        r#"{{"name": "{}", "symbol": "{}",  "image": "{}", "description": "{}", "extensions": "", "tags": []}}"#,
        name, symbol, image, description
    );

    let metadata_path = format!("{}/metadata.json", project_dir);

    match std::fs::write(&metadata_path, metadata) {
        Ok(_) => Ok(()),
        Err(e) => Err(MetadataError::GenerateError(e.to_string()))
    }
}


pub async fn upload<U: ToString + AsRef<std::path::Path>>(api_key: &str, file_path: U) -> Result<String, UploadError> {
    let file = match File::open(file_path) {
        Ok(a) => {
            a
        }
        Err(e) => {
            return Err(UploadError::ClientError(e.to_string()));
        }
    };

    tokio::task::block_in_place(|| {
        let client = reqwest::blocking::Client::new();
        let request =
            client.post(API_URL.to_owned() + "/upload")
                  .header("accept", "application/json")
                  .header("Content-Type", "*/*")
                  .bearer_auth(api_key)
                  .body(file);

        let response = match request.send() {
            Ok(a) => {
                a
            }
            Err(e) => {
                return Err(UploadError::UploadRequestError(e.to_string()));
            }
        };

        let parse: Value = match serde_json::from_str(&response.text().unwrap()) {
            Ok(a) => {
                a
            }
            Err(e) => {
                return Err(UploadError::ParseError(e.to_string()));
            }
        };

        Ok(
            parse.get("value").unwrap().get("cid").unwrap().as_str().unwrap().to_string()
        )
    })
}