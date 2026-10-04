use crate::{
    api,
    cli::Bus,
    client::ApiClient,
    error::Error,
    model::{Data, ResultData},
};
pub async fn run(client: &ApiClient, command: &Bus) -> Result<ResultData, Error> {
    match command {
        Bus::Stop { stop } => Ok(ResultData::new(Data::Stop(
            api::bus::stop(client, stop).await?,
        ))),
        Bus::Timetable {
            stop: input,
            filter,
        } => {
            let stop = api::bus::stop(client, input).await?;
            let date = crate::time::service_date(crate::time::now());
            let day = match filter.day {
                Some(crate::cli::BusDayType::Weekday) => "平日".into(),
                Some(crate::cli::BusDayType::Saturday) => "土曜".into(),
                Some(crate::cli::BusDayType::Holiday) => "日曜・休日".into(),
                None => crate::time::day(client, date, false).await?,
            };
            let mut departures = api::bus::timetable(
                client,
                &stop,
                date,
                &day,
                filter.route.as_deref(),
                filter.pole.as_deref(),
            )
            .await?;
            if let Some(after) = filter.after {
                departures.retain(|d| d.minutes >= after);
            }
            if let Some(limit) = filter.limit {
                departures.truncate(limit as usize);
            }
            let place = crate::model::Place {
                name: stop.name,
                kind: "bus".into(),
                id: stop.id,
                latitude: 0.0,
                longitude: 0.0,
                search_name: String::new(),
                codes: Vec::new(),
            };
            Ok(ResultData::new(Data::Timetable(crate::model::Timetable {
                station: place,
                scheduled: true,
                service_date: date.to_string(),
                day_type: day,
                departures,
            })))
        }
        Bus::Live {
            stop: input,
            route,
            pole,
            all,
        } => {
            let stop = api::bus::stop(client, input).await?;
            let selected_pole = pole
                .as_ref()
                .map(|name| {
                    api::choose(name, &stop.poles, |p| p.name.clone())
                        .map(|i| stop.poles[i].name.clone())
                })
                .transpose()?;
            let codes = api::bus::live_codes(client, &stop).await?;
            let service = api::bus::service(client).await;
            let mut result = ResultData::new(Data::Live(crate::model::Live {
                stop: stop.name.clone(),
                fetched_at: crate::time::now().to_rfc3339(),
                service_active: None,
                notice: String::new(),
                routes: Vec::new(),
            }));
            if let Data::Live(live) = &mut result.data
                && let Ok((active, notice)) = &service
            {
                live.service_active = Some(*active);
                live.notice = notice.clone();
            }
            if let Err(e) = service {
                result.fail("接近情報サービス設定", e);
            }
            let mut tasks = tokio::task::JoinSet::new();
            for (code, poles) in codes {
                let client = client.clone();
                tasks.spawn(async move {
                    let scope = code.clone();
                    (scope, api::bus::live_route(&client, code, poles).await)
                });
            }
            let mut routes = Vec::new();
            while let Some(item) = tasks.join_next().await {
                let (scope, item) = item.map_err(|e| Error::Network(e.to_string()))?;
                match item {
                    Ok(route) => routes.push(route),
                    Err(e) => result.fail(scope, e),
                }
            }
            routes.sort_by(|a, b| a.code.cmp(&b.code));
            if let Some(name) = route {
                let names: Vec<_> = routes
                    .iter()
                    .map(|r| r.name.clone())
                    .collect::<std::collections::BTreeSet<_>>()
                    .into_iter()
                    .collect();
                match api::choose(name, &names, Clone::clone) {
                    Ok(i) => routes.retain(|r| r.name == names[i]),
                    Err(e) if result.complete => return Err(e),
                    Err(e) => {
                        result.fail(name, e);
                        routes.clear();
                    }
                }
            }
            if let Some(pole) = selected_pole {
                for route in &mut routes {
                    route.poles.retain(|p| *p == pole);
                }
                routes.retain(|r| !r.poles.is_empty());
            }
            for route in routes {
                let client = client.clone();
                let stop = stop.clone();
                let all = *all;
                tasks.spawn(async move {
                    let mut route = route;
                    let vehicles = api::bus::vehicles(&client, &route, &stop, all).await;
                    match vehicles {
                        Ok(vehicles) => {
                            route.vehicles = vehicles;
                            (route.code.clone(), Ok(route))
                        }
                        Err(e) => (route.code, Err(e)),
                    }
                });
            }
            while let Some(item) = tasks.join_next().await {
                let (scope, item) = item.map_err(|e| Error::Network(e.to_string()))?;
                match item {
                    Ok(route) => {
                        if let Data::Live(live) = &mut result.data {
                            live.routes.push(route);
                        }
                    }
                    Err(e) => result.fail(scope, e),
                }
            }
            if let Data::Live(live) = &mut result.data {
                live.routes.sort_by(|a, b| a.code.cmp(&b.code));
            }
            result.errors.sort_by(|a, b| a.scope.cmp(&b.scope));
            Ok(result)
        }
    }
}
