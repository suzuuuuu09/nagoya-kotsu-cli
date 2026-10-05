use crate::{
    api::{choose, id, null_default, text},
    client::{ApiClient, Policy},
    error::Error,
    model::{Place, Pole, Stop},
};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize)]
struct Identifier(#[serde(deserialize_with = "id")] String);
#[derive(Default, Deserialize)]
struct StopResponse {
    #[serde(rename = "POLES", default, deserialize_with = "null_default")]
    poles: BTreeMap<String, PoleResponse>,
}
#[derive(Default, Deserialize)]
struct PoleResponse {
    #[serde(default, deserialize_with = "text")]
    name: String,
    lat: Option<f64>,
    lng: Option<f64>,
}

pub async fn names(client: &ApiClient) -> Result<Vec<Place>, Error> {
    let names: BTreeMap<String, Identifier> = client
        .json(
            client.site("/station_data/station_infos/station_name.json"),
            Policy::Master,
        )
        .await?;
    Ok(names
        .into_iter()
        .map(|(name, id)| Place {
            name: name.clone(),
            kind: "bus".into(),
            id: format!("{:0>5}", id.0),
            latitude: 0.0,
            longitude: 0.0,
            search_name: format!("{name}(名古屋市バス)"),
            codes: Vec::new(),
        })
        .collect())
}
pub async fn stop(client: &ApiClient, input: &str) -> Result<Stop, Error> {
    let input = super::place::checked_input(input, Some(crate::cli::PlaceType::Bus))?;
    let names = names(client).await?;
    let index = super::place::choose(&input, crate::cli::PlaceType::Bus, &names, |p| {
        p.name.clone()
    })?;
    let place = &names[index];
    let response: StopResponse = client
        .json(
            client.site(&format!(
                "/station_data/station_infos/stations/{}.json",
                place.id
            )),
            Policy::Master,
        )
        .await?;
    let poles = response
        .poles
        .into_iter()
        .map(|(id, p)| Pole {
            id,
            name: p.name,
            latitude: p.lat,
            longitude: p.lng,
        })
        .collect();
    Ok(Stop {
        name: place.name.clone(),
        id: place.id.clone(),
        poles,
    })
}

#[derive(Default, Deserialize)]
struct BusDiagram {
    #[serde(rename = "POLE", default, deserialize_with = "id")]
    pole: String,
    #[serde(rename = "RAILWAY", default, deserialize_with = "null_default")]
    railways: Vec<Option<String>>,
    #[serde(flatten)]
    diagram: super::Diagram,
}
pub async fn timetable(
    client: &ApiClient,
    stop: &Stop,
    date: chrono::NaiveDate,
    day: &str,
    route: Option<&str>,
    pole: Option<&str>,
) -> Result<Vec<crate::model::Departure>, Error> {
    let data: BTreeMap<String, Option<Vec<BusDiagram>>> = client
        .json(
            client.site(&format!(
                "/station_data/station_infos/diagrams/{}.json",
                stop.id
            )),
            Policy::Timetable,
        )
        .await?;
    let routes: Vec<_> = data.keys().cloned().collect();
    let selected_route = if let Some(route) = route {
        Some(routes[choose(route, &routes, Clone::clone)?].clone())
    } else {
        None
    };
    let selected_pole = if let Some(pole) = pole {
        Some(
            stop.poles[choose(pole, &stop.poles, |p| p.name.clone())?]
                .id
                .clone(),
        )
    } else {
        None
    };
    let mut result = Vec::new();
    for (line, diagrams) in data {
        if selected_route.as_ref().is_some_and(|s| s != &line) {
            continue;
        }
        for mut dia in diagrams.into_iter().flatten() {
            if selected_pole.as_ref().is_some_and(|s| s != &dia.pole) {
                continue;
            }
            let key = if dia.diagram.times.contains_key(day) {
                day
            } else if dia.diagram.times.contains_key("土曜・休日") {
                match day {
                    "土曜" => "土曜・休日",
                    "日曜・休日" => {
                        use chrono::Datelike;
                        if date.weekday() == chrono::Weekday::Sun {
                            "日曜"
                        } else {
                            "土曜・休日"
                        }
                    }
                    _ => day,
                }
            } else {
                day
            };
            if dia.diagram.platform.is_empty() {
                dia.diagram.platform = stop
                    .poles
                    .iter()
                    .find(|p| p.id == dia.pole)
                    .map(|p| p.name.clone())
                    .unwrap_or_default();
            }
            dia.diagram.railway = dia
                .railways
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join("／");
            result.extend(super::departures(&dia.diagram, &line, date, key)?);
        }
    }
    result.sort_by_key(|d| d.minutes);
    Ok(result)
}

#[derive(Default, Deserialize)]
struct Service {
    #[serde(default)]
    bus_service_active: bool,
    #[serde(default, deserialize_with = "text")]
    bus_access_info: String,
}
pub async fn service(client: &ApiClient) -> Result<(bool, String), Error> {
    let data: Vec<Service> = client
        .json(client.site("/datas/bus_service_status.json"), Policy::Live)
        .await?;
    Ok((
        data.iter().any(|s| s.bus_service_active),
        data.into_iter()
            .map(|s| s.bus_access_info)
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("\n"),
    ))
}
#[derive(Default, Deserialize)]
struct LiveStop {
    #[serde(rename = "POLES", default, deserialize_with = "null_default")]
    poles: Vec<LivePole>,
}
#[derive(Default, Deserialize)]
struct LivePole {
    #[serde(rename = "NORIBA", default, deserialize_with = "text")]
    name: String,
    #[serde(rename = "KEITOS", default, deserialize_with = "null_default")]
    routes: Vec<Identifier>,
}
pub async fn live_codes(
    client: &ApiClient,
    stop: &Stop,
) -> Result<BTreeMap<String, Vec<String>>, Error> {
    let data: LiveStop = client
        .json(
            client.site(&format!(
                "/BUS_SEKKIN/master_json/busstops/{}.json",
                stop.id
            )),
            Policy::Master,
        )
        .await?;
    let mut codes: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for pole in data.poles {
        for code in pole.routes {
            let names = codes.entry(code.0).or_default();
            if !names.contains(&pole.name) {
                names.push(pole.name.clone());
            }
        }
    }
    Ok(codes)
}
#[derive(Default, Deserialize)]
struct Keito {
    #[serde(rename = "NAME", default, deserialize_with = "text")]
    name: String,
    #[serde(rename = "TO", default, deserialize_with = "text")]
    to: String,
    #[serde(rename = "ARTICLE", default, deserialize_with = "text")]
    via: String,
    #[serde(rename = "BUSSTOPS", default, deserialize_with = "null_default")]
    stops: Vec<String>,
}
pub async fn live_route(
    client: &ApiClient,
    code: String,
    poles: Vec<String>,
) -> Result<crate::model::LiveRoute, Error> {
    let data: Keito = client
        .json(
            client.site(&format!("/BUS_SEKKIN/master_json/keitos/{code}.json")),
            Policy::Master,
        )
        .await?;
    Ok(crate::model::LiveRoute {
        code,
        name: data.name,
        destination: data.to,
        via: data.via,
        poles,
        stops: data.stops,
        vehicles: Vec::new(),
    })
}
#[derive(Default, Deserialize)]
struct PoleInfo {
    #[serde(rename = "BC", default, deserialize_with = "id")]
    code: String,
}
type Positions = BTreeMap<String, BTreeMap<String, String>>;
pub async fn vehicles(
    client: &ApiClient,
    route: &crate::model::LiveRoute,
    stop: &Stop,
    all: bool,
) -> Result<Vec<crate::model::Vehicle>, Error> {
    use crate::model::{Pass, Position, Vehicle};
    let (names, poles) = tokio::try_join!(
        names(client),
        client.json::<BTreeMap<String, PoleInfo>>(
            client.site("/BUS_SEKKIN/master_json/buspole_infos.json"),
            Policy::Master
        )
    )?;
    let data: BTreeMap<String, Box<serde_json::value::RawValue>> = client
        .json(
            client.site(&format!(
                "/BUS_SEKKIN/realtime_json/{}.json?_={}",
                route.code,
                chrono::Utc::now().timestamp_millis()
            )),
            Policy::Live,
        )
        .await?;
    let stop_name = |key: &str| {
        let id = key.split('/').next().unwrap_or(key);
        names
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.name.clone())
            .unwrap_or_else(|| id.into())
    };
    let mut current: Positions = BTreeMap::new();
    let mut history: Positions = BTreeMap::new();
    for (key, raw) in data {
        if key == "LATEST_BUS_PASS" {
            history = serde_json::from_str::<Option<Positions>>(raw.get())
                .map_err(|e| Error::Parse(e.to_string()))?
                .unwrap_or_default();
        } else if key.contains('/') {
            let value = serde_json::from_str::<Option<BTreeMap<String, String>>>(raw.get())
                .map_err(|e| Error::Parse(e.to_string()))?
                .unwrap_or_default();
            current.insert(key, value);
        }
    }
    let latest = |positions: Positions| -> Result<BTreeMap<String, (u32, String, String)>, Error> {
        let mut result = BTreeMap::new();
        for (key, vehicles) in positions {
            for (vehicle, time) in vehicles {
                if time.is_empty() {
                    continue;
                }
                let parsed = chrono::NaiveTime::parse_from_str(&time, "%H:%M:%S")
                    .map_err(|e| Error::Parse(e.to_string()))?;
                use chrono::Timelike;
                let hour = if parsed.hour() < 4 {
                    parsed.hour() + 24
                } else {
                    parsed.hour()
                };
                let seconds = hour * 3600 + parsed.minute() * 60 + parsed.second();
                let value = (
                    seconds,
                    key.clone(),
                    format!("{hour:02}:{:02}:{:02}", parsed.minute(), parsed.second()),
                );
                let entry = result.entry(vehicle).or_insert_with(|| value.clone());
                if entry.0 < seconds {
                    *entry = value;
                }
            }
        }
        Ok(result)
    };
    let current = latest(current)?;
    let history = latest(history)?;
    let target: Vec<_> = route
        .stops
        .iter()
        .enumerate()
        .filter(|(_, s)| {
            stop.poles
                .iter()
                .any(|p| route.poles.contains(&p.name) && **s == format!("{}{}", stop.id, p.id))
        })
        .map(|(i, _)| i)
        .collect();
    let mut result = Vec::new();
    let vehicle_names: std::collections::BTreeSet<_> =
        current.keys().chain(history.keys()).cloned().collect();
    for vehicle in vehicle_names {
        let position = current.get(&vehicle).map(|(_, key, _)| {
            let indices: Vec<_> = route
                .stops
                .iter()
                .enumerate()
                .filter(|(_, s)| {
                    s.get(..5)
                        .zip(poles.get(*s))
                        .is_some_and(|(id, p)| format!("{id}/{}", p.code) == *key)
                })
                .map(|(i, _)| i)
                .collect();
            if let [index] = indices.as_slice() {
                let relation = if target.is_empty() {
                    "unknown"
                } else if target.iter().any(|t| *index <= *t) {
                    "approaching"
                } else {
                    "passed"
                };
                Position {
                    from_stop: index
                        .checked_sub(1)
                        .and_then(|i| route.stops[i].get(..5))
                        .map(stop_name),
                    to_stop: Some(stop_name(key)),
                    relation: relation.into(),
                }
            } else {
                Position {
                    from_stop: None,
                    to_stop: Some(stop_name(key)),
                    relation: "unknown".into(),
                }
            }
        });
        if !all && position.as_ref().is_some_and(|p| p.relation == "passed") {
            continue;
        }
        result.push(Vehicle {
            latest_pass: history.get(&vehicle).map(|(_, key, time)| Pass {
                stop: stop_name(key),
                time: time.clone(),
            }),
            vehicle,
            current_position: position,
        });
    }
    Ok(result)
}
