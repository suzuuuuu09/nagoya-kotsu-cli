use crate::{
    api::{self, delay_cert::Record},
    cli::DelayCert,
    client::ApiClient,
    error::Error,
    model::{Data, DelayCertificate, DelayCertificates, ResultData},
};
use chrono::NaiveDateTime;
use std::collections::BTreeSet;

fn certificate(record: Record) -> Result<(NaiveDateTime, DelayCertificate), Error> {
    let datetime = record
        .delay_datetime
        .as_str()
        .ok_or_else(|| Error::InvalidResponse("証明対象日時が欠落・不正です".into()))?;
    let datetime = chrono::DateTime::parse_from_rfc3339(datetime)
        .map(|d| d.with_timezone(&chrono_tz::Asia::Tokyo).naive_local())
        .or_else(|_| NaiveDateTime::parse_from_str(datetime, "%Y-%m-%dT%H:%M:%S"))
        .map_err(|e| Error::Parse(e.to_string()))?;
    if record.delay_id.is_empty() || record.rosen_name.is_empty() {
        return Err(Error::InvalidResponse(
            "証明書IDまたは路線名が空です".into(),
        ));
    }
    let mut url =
        reqwest::Url::parse("https://www.kotsu.city.nagoya.jp/rp/subway/delay_certificate.html")
            .map_err(|e| Error::InvalidResponse(e.to_string()))?;
    url.query_pairs_mut()
        .append_pair("delay_id", &record.delay_id);
    Ok((
        datetime,
        DelayCertificate {
            line: record.rosen_name,
            date: datetime.date().to_string(),
            title: record.title,
            max_delay_time: record.max_delay_time,
            url: url.into(),
        },
    ))
}

pub async fn run(client: &ApiClient, options: &DelayCert) -> Result<ResultData, Error> {
    let records: Vec<_> = api::delay_cert::list(client)
        .await?
        .into_iter()
        .map(|body| {
            serde_json::from_str::<Record>(body.get()).map_err(|e| Error::Parse(e.to_string()))
        })
        .collect();
    let selected = if let Some(input) = &options.line {
        let names: Vec<_> = ["H_LINE", "M_LINE", "T_LINE", "S_LINE", "K_LINE"]
            .map(|id| api::line_name(id).to_owned())
            .into_iter()
            .chain(
                records
                    .iter()
                    .filter_map(|r| r.as_ref().ok().map(|r| r.rosen_name.clone())),
            )
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        Some(names[api::choose(input, &names, Clone::clone)?].clone())
    } else {
        None
    };
    let mut result = ResultData::new(Data::Empty);
    let mut certificates = Vec::new();
    for (index, record) in records.into_iter().enumerate() {
        if let Ok(record) = &record
            && selected
                .as_ref()
                .is_some_and(|line| *line != record.rosen_name)
        {
            continue;
        }
        match record.and_then(certificate) {
            Ok((datetime, cert)) => {
                if options.date.is_none_or(|date| date == datetime.date()) {
                    certificates.push((datetime, cert));
                }
            }
            Err(e) => result.fail(format!("delay-cert.certificates.{index}"), e),
        }
    }
    certificates.sort_by_key(|entry| std::cmp::Reverse(entry.0));
    if let Some(limit) = options.limit {
        certificates.truncate(limit as usize);
    }
    result.data = Data::DelayCertificates(DelayCertificates {
        fetched_at: crate::time::now().to_rfc3339(),
        certificates: certificates.into_iter().map(|(_, cert)| cert).collect(),
    });
    Ok(result)
}
