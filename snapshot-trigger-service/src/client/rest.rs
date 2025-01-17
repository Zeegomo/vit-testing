use crate::config::JobParameters;
use crate::context::State;
use crate::file_lister::FolderDump;
use reqwest::blocking::Response;
use std::io::Write;
use std::path::Path;
use thiserror::Error;

pub struct SnapshotRestClient {
    token: Option<String>,
    address: String,
}

impl SnapshotRestClient {
    pub fn new_with_token(token: String, address: String) -> Self {
        Self {
            token: Some(token),
            address,
        }
    }

    pub fn new(address: String) -> Self {
        Self {
            token: None,
            address,
        }
    }

    pub fn token(&self) -> &Option<String> {
        &self.token
    }

    pub fn address(&self) -> &String {
        &self.address
    }

    fn path<S: Into<String>>(&self, path: S) -> String {
        format!("{}/{}", self.address, path.into())
    }

    fn post<S: Into<String>>(&self, local_path: S) -> Result<Response, Error> {
        let path = self.path(local_path);
        println!("Calling: {}", path);
        let client = reqwest::blocking::Client::new();
        let request = self.set_header(client.post(&path));
        request.send().map_err(Into::into)
    }

    fn get<S: Into<String>>(&self, local_path: S) -> Result<String, Error> {
        let path = self.path(local_path);
        println!("Calling: {}", path);
        let client = reqwest::blocking::Client::new();
        let request = self.set_header(client.get(&path));
        request.send()?.text().map_err(Into::into)
    }

    fn set_header(
        &self,
        request_builder: reqwest::blocking::RequestBuilder,
    ) -> reqwest::blocking::RequestBuilder {
        if let Some(token) = &self.token {
            return request_builder.header("API-Token", token);
        }
        request_builder
    }

    pub fn list_files(&self) -> Result<FolderDump, Error> {
        serde_json::from_str(&self.get("api/job/files/list")?).map_err(Into::into)
    }

    pub fn download_snapshot<S: Into<String>, P: AsRef<Path>>(
        &self,
        id: S,
        output: P,
    ) -> Result<(), Error> {
        self.download(format!("{}/status.yaml", id.into()), output)
    }

    pub fn download_job_status<S: Into<String>, P: AsRef<Path>>(
        &self,
        id: S,
        output: P,
    ) -> Result<(), Error> {
        self.download(format!("{}/snapshot.json", id.into()), output)
    }

    pub fn download<S: Into<String>, P: AsRef<Path>>(
        &self,
        sub_location: S,
        output: P,
    ) -> Result<(), Error> {
        let content = self.get(format!("api/job/files/get/{}", sub_location.into()))?;
        let mut file = std::fs::File::create(&output)?;
        file.write_all(content.as_bytes())?;
        Ok(())
    }

    pub fn job_new(&self, params: JobParameters) -> Result<String, Error> {
        let client = reqwest::blocking::Client::new();
        let request = self.set_header(client.post("api/job/new"));
        request.json(&params).send()?.text().map_err(Into::into)
    }

    pub fn job_status<S: Into<String>>(&self, id: S) -> Result<State, Error> {
        let content = self.post(format!("api/job/status/{}", id.into()))?.text()?;
        serde_yaml::from_str(&content).map_err(Into::into)
    }

    pub fn is_up(&self) -> bool {
        if let Ok(path) = self.get("api/health") {
            if let Ok(response) = reqwest::blocking::get(&path) {
                return response.status() == reqwest::StatusCode::OK;
            }
        }
        false
    }
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("internal rest error")]
    ReqwestError(#[from] reqwest::Error),
    #[error("json response serialization error")]
    SerdeJsonError(#[from] serde_json::Error),
    #[error("yaml response serialization error")]
    SerdeYamlError(#[from] serde_yaml::Error),
    #[error("io error")]
    IoError(#[from] std::io::Error),
}
