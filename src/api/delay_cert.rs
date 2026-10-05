use crate::{
    api::id,
    client::{ApiClient, Format, Policy},
    error::Error,
};
use serde::Deserialize;
use serde_json::value::RawValue;

#[derive(Deserialize)]
pub struct Record {
    #[serde(deserialize_with = "id")]
    pub delay_id: String,
    pub rosen_name: String,
    #[serde(default)]
    pub delay_datetime: serde_json::Value,
    pub title: String,
    pub max_delay_time: String,
}

pub async fn list(client: &ApiClient) -> Result<Vec<Box<RawValue>>, Error> {
    let record = client
        .get(
            client.site("/datas/traffic_delay_certificate.json"),
            Policy::Live,
            Format::Json,
        )
        .await?;
    serde_json::from_str(record.body.trim_start_matches('\u{feff}'))
        .map_err(|e| Error::Parse(e.to_string()))
}
