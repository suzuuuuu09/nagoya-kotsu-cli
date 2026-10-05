use crate::{
    api,
    cli::Route,
    client::ApiClient,
    error::Error,
    model::{Data, ResultData, RouteResult},
};
use chrono::{NaiveDateTime, Timelike};
pub async fn run(client: &ApiClient, options: &Route) -> Result<ResultData, Error> {
    let now = crate::time::now();
    let (date, hour, minute) = if let Some(at) = &options.at {
        if let Ok(time) = chrono::NaiveTime::parse_from_str(at, "%H:%M") {
            (now.date_naive(), time.hour(), time.minute())
        } else {
            let datetime = chrono::DateTime::parse_from_rfc3339(at)
                .map(|d| d.with_timezone(&chrono_tz::Asia::Tokyo).naive_local())
                .or_else(|_| NaiveDateTime::parse_from_str(at, "%Y-%m-%dT%H:%M"))
                .or_else(|_| NaiveDateTime::parse_from_str(at, "%Y-%m-%d %H:%M"))
                .map_err(|_| {
                    Error::Arguments(
                        "--at は HH:MM または YYYY-MM-DDTHH:MM で指定してください".into(),
                    )
                })?;
            (datetime.date(), datetime.hour(), datetime.minute())
        }
    } else if options.first {
        (now.date_naive(), 4, 0)
    } else if options.last {
        (now.date_naive(), 27, 59)
    } else {
        (now.date_naive(), now.hour(), now.minute())
    };
    let kind = if options.bus != options.subway {
        Some(if options.bus {
            crate::cli::PlaceType::Bus
        } else {
            crate::cli::PlaceType::Subway
        })
    } else {
        None
    };
    for (scope, input) in [
        ("route.from", Some(&options.from)),
        ("route.to", Some(&options.to)),
        ("route.via", options.via.as_ref()),
    ] {
        if let Some(input) = input {
            api::place::checked_input(input, kind).map_err(|e| e.scoped(scope))?;
        }
    }
    let (from, to) = tokio::try_join!(
        async {
            api::route::resolve(client, &options.from, options.bus, options.subway)
                .await
                .map_err(|e| e.scoped("route.from"))
        },
        async {
            api::route::resolve(client, &options.to, options.bus, options.subway)
                .await
                .map_err(|e| e.scoped("route.to"))
        }
    )?;
    let via = if let Some(via) = &options.via {
        api::route::resolve(client, via, options.bus, options.subway)
            .await
            .map_err(|e| e.scoped("route.via"))?
            .search_name
    } else {
        String::new()
    };
    let agency = if options.bus != options.subway {
        if options.bus {
            "名古屋市バス"
        } else {
            "名古屋市地下鉄"
        }
    } else {
        "名古屋市バス/名古屋市地下鉄"
    };
    let mut mode = format!("use_agency={agency}");
    if options.slow_transfer {
        mode.push_str(",yukkuri_norikae=true");
    }
    let params = [
        ("strLicenseCode", String::new()),
        ("dteYMD", date.format("%Y/%m/%d").to_string()),
        ("intHour", hour.to_string()),
        ("intMinute", minute.to_string()),
        (
            "fromLatitude",
            (from.latitude * 3_600_000.0).round().to_string(),
        ),
        (
            "fromLongitude",
            (from.longitude * 3_600_000.0).round().to_string(),
        ),
        ("strFromEkis", format!("{},0", from.search_name)),
        ("fromDistance", "0".into()),
        (
            "toLatitude",
            (to.latitude * 3_600_000.0).round().to_string(),
        ),
        (
            "toLongitude",
            (to.longitude * 3_600_000.0).round().to_string(),
        ),
        ("strToEkis", format!("{},0", to.search_name)),
        ("toDistance", "0".into()),
        ("strPassEki1", via),
        ("strPassEki2", String::new()),
        ("strPassEki3", String::new()),
        ("strPassEki4", String::new()),
        ("intSeatType", "0".into()),
        ("blnOufuku", "false".into()),
        ("blnArrival", (!options.arrive && !options.last).to_string()),
        ("strRailMode", mode),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v))
    .collect();
    let routes = api::route::search(client, params, options.bus, options.subway).await?;
    Ok(ResultData::new(Data::Route(RouteResult {
        from,
        to,
        routes,
    })))
}
