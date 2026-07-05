use std::collections::HashMap;

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub type FetchAllPiesResponse = Vec<PieSummary>;

/// Summary information for a Trading 212 pie returned by the fetch-all-pies endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PieSummary {
    pub id: u64,
    pub cash: Decimal,
    pub dividend_details: DividendDetails,
    pub progress: Option<f64>,
    pub result: InvestmentResult,
    pub status: Option<PieStatus>,
}

/// Detailed Trading 212 pie information returned by the fetch-pie endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PieDetailed {
    pub instruments: Vec<PieInstrument>,
    pub settings: PieSettings,
}

/// Dividend totals for a Trading 212 pie.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DividendDetails {
    pub gained: Decimal,
    pub in_cash: Decimal,
    pub reinvested: Decimal,
}

/// Investment result values reported by Trading 212.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InvestmentResult {
    pub price_avg_invested_value: Option<Decimal>,
    pub price_avg_result: Option<Decimal>,
    pub price_avg_result_coef: Option<Decimal>,
    pub price_avg_value: Option<Decimal>,
    pub price_avg_value_coef: Option<Decimal>,
}

/// Instrument included in a Trading 212 pie.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PieInstrument {
    pub ticker: String,
    pub owned_quantity: Decimal,
    pub result: InvestmentResult,

    #[serde(default)]
    pub issues: Vec<PieInstrumentIssue>,
}

/// Settings for a Trading 212 pie.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
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

/// Status of a pie relative to its configured goal.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PieStatus {
    Ahead,
    OnTrack,
    Behind,

    #[serde(other)]
    Unknown,
}

/// Action Trading 212 applies to dividend cash for the pie.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DividendCashAction {
    Reinvest,
    ToAccountCash,

    #[serde(other)]
    Unknown,
}

/// Issue reported by Trading 212 for an instrument in a pie.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
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
