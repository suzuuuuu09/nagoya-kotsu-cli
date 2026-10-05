use crate::{
    api,
    cli::{Subway, SubwayDayType},
    client::ApiClient,
    error::Error,
    model::{Data, ResultData, Timetable},
};
pub async fn run(client: &ApiClient, command: &Subway) -> Result<ResultData, Error> {
    if let Subway::Station { station } = command {
        return super::places::station(client, station).await;
    }
    let now = crate::time::now();
    let (input, filter, after, limit, next) = match command {
        Subway::Station { .. } => unreachable!(),
        Subway::Timetable {
            station,
            filter,
            after,
            limit,
        } => (station, filter, *after, *limit, false),
        Subway::Next {
            station,
            filter,
            at,
            limit,
        } => (
            station,
            filter,
            Some(at.unwrap_or_else(|| crate::time::service_minutes(now))),
            Some(*limit),
            true,
        ),
    };
    let station = api::subway::station(client, input).await?;
    let date = crate::time::service_date(now);
    let day = match filter.day {
        Some(SubwayDayType::Weekday) => "平日".into(),
        Some(SubwayDayType::Holiday) => "土休日".into(),
        Some(SubwayDayType::NewYear) => "年末年始".into(),
        Some(SubwayDayType::AllNight) => "終夜".into(),
        None => crate::time::day(client, date, true).await?,
    };
    let mut departures = api::subway::timetable(
        client,
        &station,
        date,
        &day,
        filter.line.as_deref(),
        filter.direction.as_deref(),
    )
    .await?;
    if let Some(after) = after {
        departures.retain(|d| d.minutes >= after);
    }
    let mut result = ResultData::new(Data::Timetable(Timetable {
        station: station.clone(),
        scheduled: true,
        service_date: date.to_string(),
        day_type: day,
        departures,
    }));
    if next
        && matches!(&result.data,Data::Timetable(t) if t.departures.len() < limit.unwrap_or(5) as usize)
    {
        let tomorrow = date
            .succ_opt()
            .ok_or_else(|| Error::Arguments("日付が範囲外です".into()))?;
        let fetch = async {
            let day = crate::time::day(client, tomorrow, true).await?;
            api::subway::timetable(
                client,
                &station,
                tomorrow,
                &day,
                filter.line.as_deref(),
                filter.direction.as_deref(),
            )
            .await
        }
        .await;
        match fetch {
            Ok(ds) => {
                if let Data::Timetable(t) = &mut result.data {
                    t.departures.extend(ds);
                }
            }
            Err(e) => result.fail(tomorrow.to_string(), e),
        }
    }
    if let Some(limit) = limit
        && let Data::Timetable(t) = &mut result.data
    {
        t.departures.truncate(limit as usize);
    }
    Ok(result)
}
