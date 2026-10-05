use crate::{
    api::{id, text},
    client::{ApiClient, Format, Policy},
    error::Error,
    model::{Place, Route, Segment},
};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default, Deserialize)]
struct Suggestion {
    #[serde(default, deserialize_with = "id")]
    station_cd: String,
    #[serde(default, deserialize_with = "id")]
    station_div: String,
    #[serde(default, deserialize_with = "text")]
    station_name: String,
    latitude: Option<f64>,
    longitude: Option<f64>,
}
pub async fn resolve(
    client: &ApiClient,
    input: &str,
    bus: bool,
    subway: bool,
) -> Result<Place, Error> {
    let kind = if bus != subway {
        Some(if bus {
            crate::cli::PlaceType::Bus
        } else {
            crate::cli::PlaceType::Subway
        })
    } else {
        None
    };
    let key = super::place::checked_input(input, kind)?;
    let (base_name, _) = super::place::split_name(&key);
    let mut url = reqwest::Url::parse(&client.map("/optimizeapi/api/Suggest/StationInfos/json"))
        .map_err(|e| Error::Arguments(e.to_string()))?;
    url.query_pairs_mut()
        .append_pair("key", base_name)
        .append_pair(
            "div",
            if bus != subway {
                if bus { "1" } else { "2" }
            } else {
                "1,2"
            },
        )
        .append_pair("marge", "pole");
    let data: Vec<Suggestion> = client.json(url.into(), Policy::Master).await?;
    let mut seen = BTreeSet::new();
    let mut places = Vec::new();
    for s in data {
        let (kind, suffix) = match s.station_div.as_str() {
            "1" if bus || !subway => ("bus", "名古屋市バス"),
            "2" if subway || !bus => ("subway", "名古屋市地下鉄"),
            _ => continue,
        };
        if !seen.insert((s.station_cd.clone(), s.station_div)) {
            continue;
        }
        let latitude = s
            .latitude
            .ok_or_else(|| Error::InvalidResponse("駅の緯度がありません".into()))?;
        let longitude = s
            .longitude
            .ok_or_else(|| Error::InvalidResponse("駅の経度がありません".into()))?;
        if !latitude.is_finite()
            || !longitude.is_finite()
            || latitude.abs() > 90.0
            || longitude.abs() > 180.0
        {
            return Err(Error::InvalidResponse("駅の座標が範囲外です".into()));
        }
        places.push(Place {
            search_name: format!("{}({suffix})", s.station_name),
            name: s.station_name,
            kind: kind.into(),
            id: s.station_cd,
            latitude,
            longitude,
            codes: Vec::new(),
        });
    }
    let index = super::place::resolve(
        &key,
        kind,
        &places,
        |p| p.name.clone(),
        |p| {
            if p.kind == "bus" {
                crate::cli::PlaceType::Bus
            } else {
                crate::cli::PlaceType::Subway
            }
        },
    )?;
    Ok(places[index].clone())
}

pub async fn search(
    client: &ApiClient,
    params: Vec<(String, String)>,
    bus: bool,
    subway: bool,
) -> Result<Vec<Route>, Error> {
    let settings = client
        .get(
            client.map("/rp/Jassets/js/route/SETTING.js"),
            Policy::Master,
            Format::Script,
        )
        .await?;
    let guid = crate::time::script_value(&settings.body, "guid")?;
    let path = crate::time::script_value(&settings.body, "apipath")?;
    let endpoint = format!(
        "{}/PNoritsugiGateway.asmx/GetLatitudeLongitudePlusMultiIOSearchRouteDiagram",
        path.as_str()
            .ok_or_else(|| Error::Parse("経路API設定が文字列ではありません".into()))?
            .trim_end_matches('/')
    );
    let mut url =
        reqwest::Url::parse(&client.map(&endpoint)).map_err(|e| Error::Parse(e.to_string()))?;
    url.query_pairs_mut().extend_pairs(params).append_pair(
        "strGUID",
        guid.as_str()
            .ok_or_else(|| Error::Parse("GUID設定が文字列ではありません".into()))?,
    );
    let body = client.get(url.into(), Policy::Live, Format::Xml).await?;
    parse(&body.body, bus, subway)
}

type Fields = BTreeMap<String, String>;
fn field<'a>(fields: &'a Fields, key: &str) -> &'a str {
    fields.get(key).map(String::as_str).unwrap_or("")
}
fn number(fields: &Fields, key: &str) -> Result<Option<u32>, Error> {
    let s = field(fields, key).trim();
    if s.is_empty() {
        Ok(None)
    } else {
        s.parse()
            .map(Some)
            .map_err(|e| Error::Parse(format!("{key}: {e}")))
    }
}
fn datetime(value: &str) -> Result<String, Error> {
    use chrono::TimeZone;
    let time = chrono::NaiveDateTime::parse_from_str(value, "%Y/%m/%d %H:%M:%S")
        .map_err(|e| Error::Parse(e.to_string()))?;
    chrono_tz::Asia::Tokyo
        .from_local_datetime(&time)
        .single()
        .map(|d| d.to_rfc3339())
        .ok_or_else(|| Error::Parse("不正な経路日時".into()))
}
fn parse(body: &str, bus: bool, subway: bool) -> Result<Vec<Route>, Error> {
    use quick_xml::events::Event;
    let mut reader = quick_xml::Reader::from_str(body.trim_start_matches('\u{feff}'));
    reader.config_mut().trim_text(true);
    let mut stack = Vec::<String>::new();
    let mut record: Option<(String, Fields)> = None;
    let mut summaries = Vec::new();
    let mut segments = Vec::new();
    let mut status = String::new();
    let mut upstream_error = String::new();
    loop {
        let event = reader
            .read_event()
            .map_err(|e| Error::Parse(e.to_string()))?;
        match event {
            Event::Start(e) => {
                let name = String::from_utf8_lossy(e.local_name().as_ref()).into_owned();
                if matches!(name.as_str(), "HYOUKA" | "ROUTE") && record.is_none() {
                    record = Some((name.clone(), BTreeMap::new()));
                }
                stack.push(name);
            }
            Event::Text(_) | Event::CData(_) | Event::GeneralRef(_) => {
                let text = match event {
                    Event::Text(e) => e
                        .xml_content()
                        .map_err(|e| Error::Parse(e.to_string()))?
                        .into_owned(),
                    Event::CData(e) => e
                        .decode()
                        .map_err(|e| Error::Parse(e.to_string()))?
                        .into_owned(),
                    Event::GeneralRef(e) => {
                        if let Some(c) = e
                            .resolve_char_ref()
                            .map_err(|e| Error::Parse(e.to_string()))?
                        {
                            c.to_string()
                        } else {
                            quick_xml::escape::resolve_predefined_entity(
                                &e.decode().map_err(|e| Error::Parse(e.to_string()))?,
                            )
                            .ok_or_else(|| Error::Parse("不明なXML実体参照".into()))?
                            .into()
                        }
                    }
                    _ => unreachable!(),
                };
                if stack.len() >= 2
                    && stack[stack.len() - 2] == "Nstatus"
                    && stack.last().is_some_and(|s| s == "Status")
                {
                    status.push_str(&text);
                }
                if stack
                    .last()
                    .is_some_and(|s| s == "ErrorMessage" || s == "ErrorCode")
                {
                    upstream_error.push_str(&text);
                }
                if let (Some((_, fields)), Some(key)) = (&mut record, stack.last()) {
                    fields.entry(key.clone()).or_default().push_str(&text);
                }
            }
            Event::End(e) => {
                let name = String::from_utf8_lossy(e.local_name().as_ref()).into_owned();
                if record.as_ref().is_some_and(|(kind, _)| *kind == name) {
                    let (kind, fields) = record.take().unwrap();
                    if kind == "HYOUKA" {
                        summaries.push(fields);
                    } else {
                        segments.push(fields);
                    }
                }
                stack.pop();
            }
            Event::Eof => break,
            Event::DocType(_) => {
                return Err(Error::InvalidResponse(
                    "XMLのDOCTYPEは受け付けません".into(),
                ));
            }
            _ => (),
        }
    }
    if !stack.is_empty() || status.is_empty() {
        return Err(Error::Parse("経路XMLの成功状態がありません".into()));
    }
    if status != "true" {
        return Err(Error::Upstream(if upstream_error.is_empty() {
            status
        } else {
            upstream_error
        }));
    }
    let mut result = Vec::new();
    for summary in summaries {
        let route_no = field(&summary, "ROUTENO");
        if route_no.is_empty() {
            return Err(Error::Parse("経路番号がありません".into()));
        }
        let mut rides = Vec::new();
        for fields in segments.iter().filter(|s| field(s, "ROUTENO") == route_no) {
            let line = field(fields, "ROSENNAME");
            let from = field(fields, "FROMEKINAME");
            let to = field(fields, "TOEKINAME");
            let kind = if matches!(
                line,
                "東山線" | "名城線" | "名港線" | "鶴舞線" | "桜通線" | "上飯田線"
            ) {
                "subway"
            } else if line.contains("徒歩") || field(fields, "LINENAME").contains("徒歩") {
                "walk"
            } else if from.contains("(名古屋市バス)") && to.contains("(名古屋市バス)") {
                "bus"
            } else {
                "unknown"
            };
            rides.push(Segment {
                line: if line.is_empty() {
                    field(fields, "LINENAME")
                } else {
                    line
                }
                .into(),
                direction: field(fields, "DIRECTIONNAME").into(),
                from: from.into(),
                to: to.into(),
                departure: datetime(field(fields, "FROMDATETIME"))?,
                arrival: datetime(field(fields, "TODATETIME"))?,
                from_platform: field(fields, "FROMPLATFORM").into(),
                to_platform: field(fields, "TOPLATFORM").into(),
                fare_yen: number(fields, "RYOUKIN")?,
                kind: kind.into(),
            });
        }
        if rides.is_empty() {
            return Err(Error::Parse("経路の区間情報がありません".into()));
        }
        if rides.iter().any(|s| {
            s.kind != "walk" && (s.kind == "unknown" || (bus != subway && (s.kind == "bus") != bus))
        }) {
            continue;
        }
        let riding_segments = rides.iter().filter(|s| s.kind != "walk").count();
        let start = field(&summary, "TIME_BEFORE_FIRST_WALK");
        let end = field(&summary, "TIME_AFTER_LAST_WALK");
        result.push(Route {
            departure: if start.is_empty() {
                rides[0].departure.clone()
            } else {
                datetime(start)?
            },
            arrival: if end.is_empty() {
                rides.last().unwrap().arrival.clone()
            } else {
                datetime(end)?
            },
            duration_minutes: number(&summary, "MINUTE")?,
            fare_yen: number(&summary, "FARE")?,
            riding_segments,
            transfers: riding_segments.saturating_sub(1),
            segments: rides,
        });
    }
    Ok(result)
}
