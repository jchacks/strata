use std::collections::HashMap;

use chrono::{DateTime, Utc};
use reqwest::{Client, Url};
use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::DeserializeOwned};
use thiserror::Error;

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

#[derive(Debug)]
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

pub type FetchAllPiesResponse = Vec<PieSummary>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PieSummary {
    pub id: u64,
    pub cash: Decimal,
    pub dividend_details: DividendDetails,
    pub progress: Option<f64>,
    pub result: InvestmentResult,
    pub status: Option<PieStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PieDetailed {
    pub instruments: Vec<PieInstrument>,
    pub settings: PieSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DividendDetails {
    pub gained: Decimal,
    pub in_cash: Decimal,
    pub reinvested: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvestmentResult {
    pub price_avg_invested_value: Option<Decimal>,
    pub price_avg_result: Option<Decimal>,
    pub price_avg_result_coef: Option<Decimal>,
    pub price_avg_value: Option<Decimal>,
    pub price_avg_value_coef: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PieInstrument {
    pub ticker: String,
    pub owned_quantity: Decimal,
    pub result: InvestmentResult,

    #[serde(default)]
    pub issues: Vec<PieInstrumentIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PieSettings {
    pub name: String,
    pub icon: Option<String>,
    pub goal: Option<Decimal>,

    #[serde(
        default,
        deserialize_with = "deserialize_unix_timestamp_opt",
        serialize_with = "serialize_unix_timestamp_opt"
    )]
    pub creation_date: Option<DateTime<Utc>>,

    #[serde(
        default,
        deserialize_with = "deserialize_unix_timestamp_opt",
        serialize_with = "serialize_unix_timestamp_opt"
    )]
    pub end_date: Option<DateTime<Utc>>,

    pub dividend_cash_action: Option<DividendCashAction>,

    /// Maps ticker -> target share. The API may return this as null.
    pub instrument_shares: Option<HashMap<String, Decimal>>,
    pub public_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PieStatus {
    Ahead,
    OnTrack,
    Behind,

    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DividendCashAction {
    Reinvest,
    ToAccountCash,

    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PieInstrumentIssue {
    Delisted,
    Suspended,
    NoLongerAvailable,
    MaxPositionSizeReached,

    #[serde(other)]
    Unknown,
}

fn deserialize_unix_timestamp_opt<'de, D>(
    deserializer: D,
) -> Result<Option<DateTime<Utc>>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<f64>::deserialize(deserializer)?;

    match value {
        Some(seconds) => DateTime::<Utc>::from_timestamp(seconds as i64, 0)
            .ok_or_else(|| serde::de::Error::custom("invalid unix timestamp"))
            .map(Some),
        None => Ok(None),
    }
}

fn serialize_unix_timestamp_opt<S>(
    value: &Option<DateTime<Utc>>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match value {
        Some(date) => serializer.serialize_f64(date.timestamp() as f64),
        None => serializer.serialize_none(),
    }
}
