use crate::{
    api::{choose, id},
    client::{ApiClient, Format, Policy},
    error::Error,
};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::value::RawValue;
use std::collections::BTreeMap;

#[derive(Deserialize)]
pub struct Identifier(#[serde(deserialize_with = "id")] pub String);

pub struct Routes {
    pub from: String,
    pub to: String,
    pub routes: BTreeMap<String, Box<RawValue>>,
}

pub async fn master<T: DeserializeOwned>(client: &ApiClient, path: &str) -> Result<T, Error> {
    let record = client
        .get(
            client.site(&format!("/STATION_DATA/teiki/{path}")),
            Policy::Master,
            Format::Json,
        )
        .await?;
    serde_json::from_str(record.body.trim_start_matches('\u{feff}'))
        .map_err(|e| Error::Parse(e.to_string()))
}

pub fn checked_id(value: &str) -> Result<&str, Error> {
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err(Error::InvalidResponse(
            "料金データの識別子が不正です".into(),
        ));
    }
    Ok(value)
}

pub async fn get_pass_prices(
    client: &ApiClient,
    class: &str,
) -> Result<BTreeMap<String, Box<RawValue>>, Error> {
    master(client, &format!("teiki/{}.json", checked_id(class)?)).await
}

pub async fn get_bus_pass_classes(
    client: &ApiClient,
) -> Result<BTreeMap<String, Identifier>, Error> {
    master(client, "use_shi_bus.json").await
}

pub async fn get_routes(
    client: &ApiClient,
    from: &str,
    to: &str,
    route: Option<&str>,
    scope: &str,
) -> Result<Routes, Error> {
    let kind = crate::cli::PlaceType::Subway;
    let from = super::place::checked_input(from, Some(kind))
        .map_err(|e| e.scoped(format!("{scope}.from")))?;
    let to =
        super::place::checked_input(to, Some(kind)).map_err(|e| e.scoped(format!("{scope}.to")))?;
    let stations: BTreeMap<String, String> = master(client, "station.json").await?;
    let stations: Vec<_> = stations.into_iter().collect();
    let from = &stations[super::place::choose(&from, kind, &stations, |s| s.1.clone())
        .map_err(|e| e.scoped(format!("{scope}.from")))?];
    let to = &stations[super::place::choose(&to, kind, &stations, |s| s.1.clone())
        .map_err(|e| e.scoped(format!("{scope}.to")))?];
    checked_id(&to.0)?;
    let ends: BTreeMap<String, Identifier> =
        master(client, &format!("ends/{}.json", checked_id(&from.0)?)).await?;
    let mut routes = if let Some(set) = ends.get(&to.0) {
        master(client, &format!("route/{}.json", checked_id(&set.0)?)).await?
    } else {
        BTreeMap::<String, Box<RawValue>>::new()
    };
    if let Some(input) = route {
        let names: Vec<_> = routes.keys().cloned().collect();
        let name = &names[choose(input, &names, Clone::clone)?];
        routes.retain(|key, _| key == name);
    }
    Ok(Routes {
        from: from.1.clone(),
        to: to.1.clone(),
        routes,
    })
}
