use crate::{
    client::{ApiClient, Format, Policy},
    error::Error,
};
use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, TimeZone, Timelike};

pub fn now() -> chrono::DateTime<chrono_tz::Tz> {
    chrono::Utc::now().with_timezone(&chrono_tz::Asia::Tokyo)
}
pub fn service_date(now: chrono::DateTime<chrono_tz::Tz>) -> NaiveDate {
    (now - Duration::hours(4)).date_naive()
}
pub fn service_minutes(n: chrono::DateTime<chrono_tz::Tz>) -> u32 {
    (if n.hour() < 4 {
        n.hour() + 24
    } else {
        n.hour()
    }) * 60
        + n.minute()
}
pub fn departure(date: NaiveDate, minutes: u32) -> String {
    let instant = date.and_time(chrono::NaiveTime::MIN) + Duration::minutes(i64::from(minutes));
    chrono_tz::Asia::Tokyo
        .from_local_datetime(&instant)
        .single()
        .map(|d| d.to_rfc3339())
        .unwrap_or_default()
}

pub fn script_value(source: &str, key: &str) -> Result<serde_json::Value, Error> {
    let tail = source
        .split_once(&format!("{key}:"))
        .ok_or_else(|| Error::Parse(format!("設定項目 {key} がありません")))?
        .1
        .trim_start();
    let end = if tail.starts_with('[') {
        tail.find(']').map(|i| i + 1)
    } else if let Some(tail) = tail.strip_prefix('"') {
        tail.find('"').map(|i| i + 2)
    } else {
        None
    }
    .ok_or_else(|| Error::Parse(format!("設定項目 {key} の形式が不正です")))?;
    serde_json::from_str(&tail[..end]).map_err(|e| Error::Parse(e.to_string()))
}
pub async fn day(client: &ApiClient, date: NaiveDate, subway: bool) -> Result<String, Error> {
    let path = if subway {
        "/rp/Jassets/js/subway/subway_setting.js"
    } else {
        "/rp/Jassets/js/bus/station_setting.js"
    };
    let settings = client
        .get(client.site(path), Policy::Master, Format::Script)
        .await?;
    if subway {
        for (prefix, kind) in [("allnight", "終夜"), ("new_year", "年末年始")] {
            let start = script_value(&settings.body, &format!("{prefix}_start"))?;
            let end = script_value(&settings.body, &format!("{prefix}_end"))?;
            let parse = |v: &serde_json::Value| {
                NaiveDateTime::parse_from_str(v.as_str().unwrap_or(""), "%Y/%m/%d %H:%M:%S")
                    .map_err(|e| Error::Parse(e.to_string()))
            };
            let start = parse(&start)?;
            let end = parse(&end)?;
            if start < end
                && date >= (start - Duration::hours(4)).date()
                && date < (end - Duration::hours(4)).date()
            {
                return Ok(kind.into());
            }
        }
    }
    let special = script_value(&settings.body, "special_holiday_dates")?;
    let formatted = date.format("%Y/%m/%d").to_string();
    if !special.is_array() {
        return Err(Error::Parse("休日設定が配列ではありません".into()));
    }
    let special_holiday = special
        .as_array()
        .is_some_and(|a| a.iter().any(|d| d.as_str() == Some(&formatted)));
    let holidays = include_str!("../data/holidays.txt");
    let first = holidays
        .lines()
        .next()
        .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());
    let last = holidays
        .lines()
        .last()
        .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());
    if !special_holiday
        && (first.is_none_or(|d| date.year() < d.year())
            || last.is_none_or(|d| date.year() > d.year()))
    {
        return Err(Error::Arguments(
            "祝日カレンダーの対象年外です。--day を指定してください".into(),
        ));
    }
    let holiday = special_holiday
        || holidays.lines().any(|d| d == date.to_string())
        || date.weekday() == chrono::Weekday::Sun;
    Ok(if holiday {
        if subway {
            "土休日"
        } else {
            "日曜・休日"
        }
    } else if date.weekday() == chrono::Weekday::Sat {
        if subway { "土休日" } else { "土曜" }
    } else {
        "平日"
    }
    .into())
}
