use crate::{
    api::{Diagram, choose, departures, id, null_default, text},
    client::{ApiClient, Policy},
    error::Error,
    model::{Departure, Place},
};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Default, Deserialize)]
struct Station {
    #[serde(default, deserialize_with = "id")]
    id: String,
    #[serde(default, deserialize_with = "text")]
    name: String,
    #[serde(default, deserialize_with = "null_default")]
    codes: Vec<String>,
}
pub async fn station(client: &ApiClient, input: &str) -> Result<Place, Error> {
    let input = super::place::checked_input(input, Some(crate::cli::PlaceType::Subway))?;
    let names: Vec<Station> = client
        .json(
            client.site("/station_data/station_subway_infos/station_master.json"),
            Policy::Master,
        )
        .await?;
    let i = super::place::choose(&input, crate::cli::PlaceType::Subway, &names, |s| {
        s.name.clone()
    })?;
    let s = &names[i];
    Ok(Place {
        name: s.name.clone(),
        kind: "subway".into(),
        id: s.id.clone(),
        latitude: 0.0,
        longitude: 0.0,
        search_name: format!("{}(名古屋市地下鉄)", s.name),
        codes: s.codes.clone(),
    })
}
pub async fn timetable(
    client: &ApiClient,
    station: &Place,
    date: chrono::NaiveDate,
    day: &str,
    line: Option<&str>,
    direction: Option<&str>,
) -> Result<Vec<Departure>, Error> {
    let data: BTreeMap<String, Option<Vec<Diagram>>> = client
        .json(
            client.site(&format!(
                "/station_data/station_subway_infos/diagrams/{}.json",
                station.id
            )),
            Policy::Timetable,
        )
        .await?;
    let lines: Vec<_> = data.keys().cloned().collect();
    let selected = if let Some(line) = line {
        Some(lines[choose(line, &lines, Clone::clone)?].clone())
    } else {
        None
    };
    let diagrams: Vec<_> = data
        .iter()
        .filter(|(l, _)| selected.as_ref().is_none_or(|s| s == *l))
        .flat_map(|(l, ds)| ds.iter().flatten().map(move |d| (l, d)))
        .collect();
    let directions: Vec<_> = diagrams
        .iter()
        .flat_map(|(_, d)| [d.direction.clone(), d.railway.clone()])
        .filter(|s| !s.is_empty())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let selected_direction = if let Some(value) = direction {
        Some(directions[choose(value, &directions, Clone::clone)?].clone())
    } else {
        None
    };
    let mut result = Vec::new();
    for (line, dia) in diagrams {
        if selected_direction
            .as_ref()
            .is_some_and(|d| d != &dia.direction && d != &dia.railway)
        {
            continue;
        }
        result.extend(departures(dia, line, date, day)?);
    }
    result.sort_by_key(|d| d.minutes);
    Ok(result)
}
