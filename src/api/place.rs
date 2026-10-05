use crate::{
    api::{self, id, null_default, text},
    cli::PlaceType,
    client::{ApiClient, Policy},
    error::Error,
    model::{PlaceLocation, SearchPlace},
};
use serde::Deserialize;
use serde_json::value::RawValue;
use std::collections::{BTreeMap, BTreeSet};

const COORDINATES: &str = "/STATION_DATA/station_infos/station_latlng.json";

#[derive(Deserialize)]
pub struct Station {
    #[serde(default, deserialize_with = "text")]
    pub name: String,
    #[serde(default, deserialize_with = "null_default")]
    pub codes: Vec<String>,
}

pub async fn stations(client: &ApiClient) -> Result<Vec<Station>, Error> {
    let stations: Vec<Station> = client
        .json(
            client.site("/station_data/station_subway_infos/station_master.json"),
            Policy::Master,
        )
        .await?;
    if stations.iter().any(|s| api::normalize(&s.name).is_empty()) {
        return Err(Error::InvalidResponse("駅マスターの名前が空です".into()));
    }
    Ok(stations)
}

#[derive(Deserialize)]
struct CoordinateRecord {
    name: String,
    lat: Option<Box<RawValue>>,
    lng: Option<Box<RawValue>>,
}

pub async fn coordinates(client: &ApiClient) -> Result<Vec<Box<RawValue>>, Error> {
    client.json(client.site(COORDINATES), Policy::Master).await
}

fn checked_coordinates(
    lat: Option<&RawValue>,
    lng: Option<&RawValue>,
) -> Result<Option<(f64, f64)>, Error> {
    let number = |value: Option<&RawValue>| -> Result<Option<f64>, Error> {
        value
            .map(|v| {
                serde_json::from_str(v.get())
                    .map_err(|_| Error::InvalidResponse("座標が数値ではありません".into()))
            })
            .transpose()
            .map(Option::flatten)
    };
    match (number(lat)?, number(lng)?) {
        (None, None) => Ok(None),
        (Some(lat), Some(lng))
            if lat.is_finite() && lng.is_finite() && lat.abs() <= 90.0 && lng.abs() <= 180.0 =>
        {
            Ok(Some((lat, lng)))
        }
        _ => Err(Error::InvalidResponse("座標が欠落・範囲外です".into())),
    }
}

pub fn qualified_name(name: &str, kind: PlaceType) -> String {
    format!("{name}({})", kind.suffix())
}

pub fn split_name(input: &str) -> (&str, Option<PlaceType>) {
    for kind in [PlaceType::Bus, PlaceType::Subway] {
        if let Some(name) = input.strip_suffix(&format!("({})", kind.suffix())) {
            return (name, Some(kind));
        }
    }
    (input, None)
}

pub fn checked_input(input: &str, kind: Option<PlaceType>) -> Result<String, Error> {
    let input = api::normalize(input);
    let (name, suffix) = split_name(&input);
    if api::normalize(name).is_empty() {
        return Err(Error::Arguments("地点名を空にできません".into()));
    }
    if kind
        .zip(suffix)
        .is_some_and(|(kind, suffix)| kind != suffix)
    {
        return Err(Error::Arguments(
            "完全修飾名と地点種別が矛盾しています".into(),
        ));
    }
    Ok(input)
}

pub struct Entry {
    pub name: String,
    pub kind: PlaceType,
    pub qualified_name: String,
    coordinates: Option<Result<Option<(f64, f64)>, Error>>,
}
impl Entry {
    fn new(name: String, kind: PlaceType) -> Self {
        Self {
            qualified_name: qualified_name(&name, kind),
            name,
            kind,
            coordinates: None,
        }
    }
    fn merge_coordinates(&mut self, value: Result<Option<(f64, f64)>, Error>) {
        self.coordinates = Some(match self.coordinates.take() {
            None => value,
            Some(Ok(previous)) if value.as_ref().ok() == Some(&previous) => Ok(previous),
            Some(Err(error)) => Err(error),
            _ => Err(Error::InvalidResponse(
                "同じ施設の座標が矛盾・不正です".into(),
            )),
        });
    }
    pub fn location(&self) -> Result<PlaceLocation, Error> {
        let (latitude, longitude) = self
            .coordinate()?
            .ok_or_else(|| Error::InvalidResponse("施設の座標がありません".into()))?;
        Ok(PlaceLocation {
            name: self.name.clone(),
            qualified_name: self.qualified_name.clone(),
            kind: self.kind.as_str().into(),
            latitude,
            longitude,
        })
    }
    pub fn coordinate(&self) -> Result<Option<(f64, f64)>, Error> {
        self.coordinates.clone().unwrap_or(Ok(None))
    }
}

pub struct Index {
    pub entries: Vec<Entry>,
    pub errors: Vec<(Option<String>, Error)>,
}
impl Index {
    pub fn resolve(&self, input: &str, kind: Option<PlaceType>) -> Result<&Entry, Error> {
        let (name, suffix) = split_name(input);
        let name = api::normalize(name);
        let kind = kind.or(suffix);
        let candidates: Vec<_> = self
            .entries
            .iter()
            .filter(|entry| kind.is_none_or(|kind| kind == entry.kind))
            .collect();
        let i = api::choose(&name, &candidates, |entry| entry.name.clone()).map_err(|error| {
            let names = match &error {
                Error::Ambiguous { .. } => {
                    let exact = candidates.iter().any(|e| api::normalize(&e.name) == name);
                    candidates
                        .iter()
                        .filter(|e| {
                            if exact {
                                api::normalize(&e.name) == name
                            } else {
                                api::normalize(&e.name).contains(&name)
                            }
                        })
                        .map(|e| e.qualified_name.clone())
                        .collect::<Vec<_>>()
                }
                _ => candidates
                    .iter()
                    .take(12)
                    .map(|e| e.qualified_name.clone())
                    .collect(),
            }
            .join("、");
            match error {
                Error::Ambiguous { .. } => Error::Ambiguous {
                    name: input.into(),
                    candidates: names,
                },
                Error::NotFound { .. } => Error::NotFound {
                    name: input.into(),
                    candidates: names,
                },
                error => error,
            }
        })?;
        Ok(candidates[i])
    }
}

pub async fn place_index(client: &ApiClient) -> Result<Index, Error> {
    let (coords, buses, subways) = tokio::join!(
        coordinates(client),
        api::bus::names(client),
        stations(client)
    );
    let coords = coords?;
    let buses = buses?;
    let subways = subways?;
    let mut entries = BTreeMap::<String, Vec<Entry>>::new();
    for (name, kind) in buses
        .into_iter()
        .map(|p| (p.name, PlaceType::Bus))
        .chain(subways.into_iter().map(|s| (s.name, PlaceType::Subway)))
    {
        if api::normalize(&name).is_empty() {
            return Err(Error::InvalidResponse(
                "名前マスターの地点名が空です".into(),
            ));
        }
        let bucket = entries
            .entry(qualified_name(&api::normalize(&name), kind))
            .or_default();
        if !bucket.iter().any(|entry| entry.name == name) {
            bucket.push(Entry::new(name, kind));
        }
    }
    let bus_names: BTreeSet<_> = entries
        .values()
        .flatten()
        .filter(|e| e.kind == PlaceType::Bus)
        .map(|e| api::normalize(&e.name))
        .collect();
    let subway_names: BTreeSet<_> = entries
        .values()
        .flatten()
        .filter(|e| e.kind == PlaceType::Subway)
        .map(|e| api::normalize(&e.name))
        .collect();
    let mut errors = Vec::new();
    for body in coords {
        let record: CoordinateRecord = match serde_json::from_str(body.get()) {
            Ok(record) => record,
            Err(_) => {
                errors.push((
                    None,
                    Error::InvalidResponse("座標レコードの形式が不正です".into()),
                ));
                continue;
            }
        };
        let normalized = api::normalize(&record.name);
        let (name, suffix) = split_name(&normalized);
        if name.is_empty() {
            errors.push((None, Error::InvalidResponse("座標の地点名が空です".into())));
            continue;
        }
        let kind = match suffix {
            Some(kind) => kind,
            None => match (bus_names.contains(name), subway_names.contains(name)) {
                (true, false) => PlaceType::Bus,
                (false, true) => PlaceType::Subway,
                (true, true) => {
                    errors.push((
                        Some(name.into()),
                        Error::InvalidResponse("座標の地点種別を判定できません".into()),
                    ));
                    continue;
                }
                (false, false) => continue,
            },
        };
        let bucket = entries
            .entry(qualified_name(name, kind))
            .or_insert_with(|| vec![Entry::new(name.into(), kind)]);
        for entry in bucket {
            entry.merge_coordinates(checked_coordinates(
                record.lat.as_deref(),
                record.lng.as_deref(),
            ));
        }
    }
    Ok(Index {
        entries: entries.into_values().flatten().collect(),
        errors,
    })
}

pub fn distance_m(from: &PlaceLocation, to: &PlaceLocation) -> f64 {
    const EARTH_RADIUS_M: f64 = 6_371_000.0;
    let lat = ((to.latitude - from.latitude).to_radians() / 2.0).sin();
    let lng = ((to.longitude - from.longitude).to_radians() / 2.0).sin();
    let a =
        lat * lat + from.latitude.to_radians().cos() * to.latitude.to_radians().cos() * lng * lng;
    2.0 * EARTH_RADIUS_M * a.clamp(0.0, 1.0).sqrt().asin()
}

#[derive(Deserialize)]
struct Suggestion {
    #[serde(default, deserialize_with = "id")]
    station_cd: String,
    #[serde(default, deserialize_with = "id")]
    station_div: String,
    #[serde(default, deserialize_with = "text")]
    station_name: String,
    station_num: Option<Box<RawValue>>,
    line_name: Option<Box<RawValue>>,
    latitude: Option<Box<RawValue>>,
    longitude: Option<Box<RawValue>>,
}
struct Candidate {
    entry: Entry,
    codes: BTreeSet<String>,
    lines: BTreeSet<String>,
    valid_name: bool,
}
pub struct Suggestions {
    pub places: Vec<SearchPlace>,
    pub errors: Vec<(String, Error)>,
}

pub async fn suggest(
    client: &ApiClient,
    query: &str,
    kind: PlaceType,
) -> Result<Suggestions, Error> {
    let div = match kind {
        PlaceType::Bus => "1",
        PlaceType::Subway => "2",
    };
    let mut url = reqwest::Url::parse(&client.map("/optimizeapi/api/Suggest/StationInfos/json"))
        .map_err(|e| Error::Arguments(e.to_string()))?;
    url.query_pairs_mut()
        .append_pair("key", query)
        .append_pair("div", div)
        .append_pair("marge", "pole")
        .append_pair("next", "");
    let records: Vec<Box<RawValue>> = client.json(url.into(), Policy::Live).await?;
    let mut candidates = BTreeMap::<String, Candidate>::new();
    let mut errors = Vec::new();
    for (index, body) in records.into_iter().enumerate() {
        let scope = format!("search.{}.results.{index}", kind.as_str());
        let s: Suggestion = match serde_json::from_str(body.get()) {
            Ok(s) => s,
            Err(_) => {
                errors.push((
                    scope,
                    Error::InvalidResponse("検索候補の形式が不正です".into()),
                ));
                continue;
            }
        };
        if s.station_cd.trim().is_empty()
            || api::normalize(&s.station_name).is_empty()
            || s.station_div != div
        {
            errors.push((
                scope,
                Error::InvalidResponse("検索候補の名前・識別子・種別が不正です".into()),
            ));
            continue;
        }
        let candidate = candidates.entry(s.station_cd).or_insert_with(|| Candidate {
            entry: Entry::new(s.station_name.clone(), kind),
            codes: BTreeSet::new(),
            lines: BTreeSet::new(),
            valid_name: true,
        });
        if api::normalize(&candidate.entry.name) != api::normalize(&s.station_name) {
            candidate.valid_name = false;
        }
        candidate.entry.merge_coordinates(checked_coordinates(
            s.latitude.as_deref(),
            s.longitude.as_deref(),
        ));
        for (value, separator, target, field) in [
            (s.station_num.as_deref(), '|', &mut candidate.codes, "codes"),
            (s.line_name.as_deref(), ',', &mut candidate.lines, "lines"),
        ] {
            if field == "codes" && kind == PlaceType::Bus {
                continue;
            }
            let value = value
                .map(|v| serde_json::from_str::<Option<String>>(v.get()))
                .transpose();
            match value {
                Ok(value) => target.extend(
                    value
                        .flatten()
                        .unwrap_or_default()
                        .split(separator)
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_owned),
                ),
                Err(_) => errors.push((
                    format!(
                        "search.{}.{}.{}",
                        kind.as_str(),
                        candidate.entry.qualified_name,
                        field
                    ),
                    Error::InvalidResponse("検索候補の駅記号・路線情報が不正です".into()),
                )),
            }
        }
    }
    let mut places = Vec::new();
    for candidate in candidates.into_values() {
        let scope = format!(
            "search.{}.{}",
            kind.as_str(),
            candidate.entry.qualified_name
        );
        if !candidate.valid_name {
            errors.push((
                scope,
                Error::InvalidResponse("同じ識別子の施設名が矛盾しています".into()),
            ));
            continue;
        }
        let coords = match candidate.entry.coordinate() {
            Ok(coords) => coords,
            Err(error) => {
                errors.push((scope, error));
                None
            }
        };
        places.push(SearchPlace {
            name: candidate.entry.name,
            qualified_name: candidate.entry.qualified_name,
            kind: kind.as_str().into(),
            codes: candidate.codes.into_iter().collect(),
            lines: candidate.lines.into_iter().collect(),
            latitude: coords.map(|c| c.0),
            longitude: coords.map(|c| c.1),
        });
    }
    Ok(Suggestions { places, errors })
}

pub fn station_coordinates(
    records: Vec<Box<RawValue>>,
    name: &str,
) -> Result<Option<(f64, f64)>, Error> {
    let name = api::normalize(name);
    let qualified = qualified_name(&name, PlaceType::Subway);
    let mut entry = Entry::new(name.clone(), PlaceType::Subway);
    for body in records {
        let Ok(record) = serde_json::from_str::<CoordinateRecord>(body.get()) else {
            continue;
        };
        let record_name = api::normalize(&record.name);
        if record_name == qualified {
            entry.merge_coordinates(checked_coordinates(
                record.lat.as_deref(),
                record.lng.as_deref(),
            ));
        } else if record_name == name {
            // 2マスターだけでは、接尾辞のない同名バス停との区別を確認できない。
            entry.merge_coordinates(Err(Error::InvalidResponse(
                "座標の地点種別を確認できません".into(),
            )));
        }
    }
    entry.coordinate()
}
