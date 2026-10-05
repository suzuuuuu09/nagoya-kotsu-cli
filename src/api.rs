use crate::{
    client::{ApiClient, Policy},
    error::Error,
    model,
};
use serde::{Deserialize, Deserializer};
use unicode_normalization::UnicodeNormalization;
pub mod bus;
pub mod delay_cert;
pub mod place;
pub mod route;
pub mod subway;
pub mod teiki;

#[derive(Deserialize)]
#[serde(untagged)]
pub enum Minute {
    Number(u32),
    Text(String),
}
impl Minute {
    fn value(&self) -> Result<u32, Error> {
        match self {
            Self::Number(n) => Ok(*n),
            Self::Text(s) => s
                .parse()
                .map_err(|e| Error::Parse(format!("不正な時刻表の分: {e}"))),
        }
    }
}
type Hourly<T> = std::collections::BTreeMap<String, Option<Vec<Option<T>>>>;
type Daily<T> = std::collections::BTreeMap<String, Hourly<T>>;
#[derive(Default, Deserialize)]
pub struct Diagram {
    #[serde(rename = "DIAGRAM", default, deserialize_with = "null_default")]
    pub times: Daily<Minute>,
    #[serde(rename = "DESTINATION", default, deserialize_with = "null_default")]
    pub destinations: Daily<String>,
    #[serde(
        rename = "NOTES",
        alias = "NOTE",
        default,
        deserialize_with = "null_default"
    )]
    pub notes: std::collections::BTreeMap<String, Option<Vec<Option<String>>>>,
    #[serde(rename = "RAILWAY", default, deserialize_with = "text")]
    pub railway: String,
    #[serde(rename = "DIRECTION", default, deserialize_with = "text")]
    pub direction: String,
    #[serde(
        rename = "PLATFORM",
        alias = "POLENAME",
        default,
        deserialize_with = "text"
    )]
    pub platform: String,
}

pub fn departures(
    dia: &Diagram,
    line: &str,
    date: chrono::NaiveDate,
    day: &str,
) -> Result<Vec<model::Departure>, Error> {
    let Some(times) = dia.times.get(day) else {
        return Ok(Vec::new());
    };
    let mut notes = std::collections::BTreeMap::new();
    for note in dia
        .notes
        .get(day)
        .into_iter()
        .flatten()
        .flatten()
        .flatten()
        .flat_map(|note| {
            if note.contains('…') {
                note.split_whitespace().collect::<Vec<_>>()
            } else {
                vec![note.as_str()]
            }
        })
    {
        if let Some((symbol, destination)) = note.split_once('…') {
            notes.insert(
                if symbol.trim() == "無印" {
                    "".to_owned()
                } else {
                    symbol.trim().into()
                },
                destination.trim().trim_end_matches('行').to_owned(),
            );
        } else if let Some(first) = note.chars().next() {
            notes.insert(
                first.to_string().trim().to_owned(),
                note[first.len_utf8()..]
                    .trim()
                    .trim_end_matches('行')
                    .to_owned(),
            );
        }
    }
    let mut result = Vec::new();
    for (hour, minutes) in times {
        let hour: u32 = hour
            .parse()
            .map_err(|_| Error::Parse(format!("不正な時刻表の時間 {hour}")))?;
        if hour > 27 {
            return Err(Error::Parse("時刻表の時間が範囲外です".into()));
        }
        let hour = if hour < 4 { hour + 24 } else { hour };
        for (index, minute) in minutes.iter().flatten().enumerate() {
            let Some(minute) = minute else { continue };
            let minute = minute.value()?;
            if minute > 59 {
                return Err(Error::Parse("時刻表の分が範囲外です".into()));
            }
            let symbol = dia
                .destinations
                .get(day)
                .and_then(|d| {
                    d.get(&(hour % 24).to_string())
                        .or_else(|| d.get(&hour.to_string()))
                })
                .and_then(|a| a.as_ref())
                .and_then(|a| a.get(index))
                .and_then(|d| d.as_deref())
                .unwrap_or("")
                .trim()
                .to_owned();
            let destination = notes.get(&symbol).cloned().or_else(|| {
                if symbol.is_empty()
                    && matches!(line, "東山線" | "鶴舞線" | "桜通線" | "上飯田線")
                    && dia.railway.ends_with("方面")
                {
                    dia.railway
                        .trim_end_matches("方面")
                        .split('・')
                        .next_back()
                        .map(str::to_owned)
                } else {
                    None
                }
            });
            let minute = hour * 60 + minute;
            result.push(model::Departure {
                time: format!("{:02}:{:02}", minute / 60, minute % 60),
                departure: crate::time::departure(date, minute),
                service_date: date.to_string(),
                day_type: day.into(),
                line: line.into(),
                direction: dia.railway.clone(),
                platform: dia.platform.clone(),
                destination,
                destination_symbol: symbol,
                minutes: minute,
            });
        }
    }
    result.sort_by_key(|d| d.minutes);
    Ok(result)
}

pub fn normalize(value: &str) -> String {
    value.trim().nfkc().collect()
}
pub fn text<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    Option::<String>::deserialize(d).map(Option::unwrap_or_default)
}
pub fn id<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    let raw = Box::<serde_json::value::RawValue>::deserialize(d)?;
    if raw.get() == "null" {
        return Ok(String::new());
    }
    if raw.get().starts_with('"') {
        serde_json::from_str(raw.get()).map_err(serde::de::Error::custom)
    } else if raw.get().bytes().all(|b| b.is_ascii_digit()) {
        Ok(raw.get().to_owned())
    } else {
        Err(serde::de::Error::custom(
            "識別子は文字列または整数の字句である必要があります",
        ))
    }
}
pub fn null_default<'de, D: Deserializer<'de>, T: Deserialize<'de> + Default>(
    d: D,
) -> Result<T, D::Error> {
    Option::<T>::deserialize(d).map(Option::unwrap_or_default)
}
pub fn choose<T>(input: &str, values: &[T], name: impl Fn(&T) -> String) -> Result<usize, Error> {
    let input = normalize(input);
    let names: Vec<_> = values.iter().map(&name).collect();
    let mut candidates: Vec<_> = names
        .iter()
        .enumerate()
        .filter(|(_, n)| normalize(n) == input)
        .map(|(i, _)| i)
        .collect();
    if candidates.is_empty() {
        candidates = names
            .iter()
            .enumerate()
            .filter(|(_, n)| normalize(n).contains(&input))
            .map(|(i, _)| i)
            .collect();
    }
    match candidates.as_slice() {
        [i] => Ok(*i),
        [] => Err(Error::NotFound {
            name: input,
            candidates: names.into_iter().take(12).collect::<Vec<_>>().join("、"),
        }),
        _ => Err(Error::Ambiguous {
            name: input,
            candidates: candidates
                .iter()
                .map(|&i| names[i].clone())
                .collect::<Vec<_>>()
                .join("、"),
        }),
    }
}

#[derive(Default, Deserialize)]
struct Traffic {
    #[serde(default, deserialize_with = "text")]
    rosen_id: String,
    #[serde(default, deserialize_with = "text")]
    traffic_title: String,
    #[serde(default, deserialize_with = "text")]
    traffic_message: String,
    #[serde(default)]
    create_datetime: Option<String>,
}
pub async fn status(client: &ApiClient) -> Result<model::Status, Error> {
    let data: Vec<Traffic> = client
        .json(client.site("/datas/latest_traffic.json"), Policy::Live)
        .await?;
    let records = data
        .into_iter()
        .map(|d| model::StatusRecord {
            line: line_name(&d.rosen_id).into(),
            title: d.traffic_title,
            message: d.traffic_message,
            created_at: d.create_datetime,
        })
        .collect();
    Ok(model::Status {
        fetched_at: chrono::Utc::now()
            .with_timezone(&chrono_tz::Asia::Tokyo)
            .to_rfc3339(),
        records,
    })
}
pub fn line_name(id: &str) -> &str {
    match id {
        "H_LINE" => "東山線",
        "M_LINE" => "名城線・名港線",
        "T_LINE" => "鶴舞線",
        "S_LINE" => "桜通線",
        "K_LINE" => "上飯田線",
        "B_LINE" => "市バス",
        name => name,
    }
}
