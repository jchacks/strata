use reqwest::{Client, Url};
use serde::de::DeserializeOwned;
use thiserror::Error;

use crate::trading212::types::{FetchAllPiesResponse, PieDetailed};

#[derive(Debug, Error)]
pub enum Trading212Error {
    #[error("invalid base URL: {0}")]
    InvalidBaseUrl(#[from] url::ParseError),

    #[error("HTTP request failed: {0}")]
    Request(#[from] reqwest::Error),

    #[error("Trading 212 API returned an error: status {status}, body: {body}")]
    Api {
        status: reqwest::StatusCode,
        body: String,
    },
}

#[derive(Debug, Clone)]
pub struct Trading212Client {
    http: Client,
    base_url: Url,
    api_key: String,
    api_secret: String,
}

impl Trading212Client {
    pub fn new(
        base_url: impl AsRef<str>,
        api_key: String,
        api_secret: String,
    ) -> Result<Self, Trading212Error> {
        Ok(Self {
            http: Client::new(),
            base_url: Url::parse(base_url.as_ref())?,
            api_key,
            api_secret,
        })
    }

    async fn send_get<T>(&self, path: &str) -> Result<T, Trading212Error>
    where
        T: DeserializeOwned,
    {
        let url = self.base_url.join(path)?;

        let response = self
            .http
            .get(url)
            .basic_auth(&self.api_key, Some(&self.api_secret))
            .send()
            .await?;
        let status = response.status();

        if !status.is_success() {
            let body = response.text().await?;
            return Err(Trading212Error::Api { status, body });
        }

        let data = response.json::<T>().await?;
        Ok(data)
    }

    pub async fn get_pies(&self) -> Result<FetchAllPiesResponse, Trading212Error> {
        self.send_get("equity/pies").await
    }

    pub async fn get_pie(&self, pie_id: u64) -> Result<PieDetailed, Trading212Error> {
        self.send_get(&format!("equity/pies/{pie_id}")).await
    }
}
