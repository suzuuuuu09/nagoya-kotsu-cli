use crate::{
    api::{self, place},
    cli::{Location, Nearby, PlaceType, Search},
    client::ApiClient,
    error::Error,
    model::{Data, NearbyPlace, NearbyResult, ResultData, SearchResult, StationInfo},
};

pub async fn location(client: &ApiClient, options: &Location) -> Result<ResultData, Error> {
    let input = place::checked_input(&options.place, options.kind)?;
    let index = place::place_index(client).await?;
    let entry = index.resolve(&input, options.kind)?;
    let mut result = ResultData::new(Data::Location(entry.location()?));
    let selected_name = api::normalize(&entry.name);
    for (name, error) in index.errors {
        if name.is_some_and(|name| name == selected_name) {
            result.fail("location.coordinates", error);
        }
    }
    Ok(result)
}

pub async fn nearby(client: &ApiClient, options: &Nearby) -> Result<ResultData, Error> {
    let input = place::checked_input(&options.place, options.origin_type)?;
    let index = place::place_index(client).await?;
    let origin = index.resolve(&input, options.origin_type)?.location()?;
    let mut result = ResultData::new(Data::Empty);
    let mut places = Vec::new();
    for entry in &index.entries {
        if entry.qualified_name == origin.qualified_name
            || options.kind.is_some_and(|kind| kind != entry.kind)
        {
            continue;
        }
        match entry.location() {
            Ok(location) => {
                let distance = place::distance_m(&origin, &location);
                if options
                    .radius
                    .is_none_or(|radius| distance <= radius as f64)
                {
                    places.push(NearbyPlace {
                        location,
                        distance_m: distance.round() as u32,
                    });
                }
            }
            Err(error) => result.fail(format!("nearby.{}", entry.qualified_name), error),
        }
    }
    for (_, error) in index.errors {
        result.fail("nearby.coordinates", error);
    }
    places.sort_by(|a, b| {
        a.distance_m
            .cmp(&b.distance_m)
            .then_with(|| a.location.qualified_name.cmp(&b.location.qualified_name))
    });
    places.truncate(options.limit as usize);
    result.data = Data::Nearby(NearbyResult { origin, places });
    Ok(result)
}

pub async fn search(client: &ApiClient, options: &Search) -> Result<ResultData, Error> {
    let query = api::normalize(&options.query);
    if query.is_empty() {
        return Err(Error::Arguments("検索語を空にできません".into()));
    }
    let mut result = ResultData::new(Data::Empty);
    let mut places = Vec::new();
    let mut successful = false;
    for kind in [PlaceType::Bus, PlaceType::Subway] {
        if options.kind.is_some_and(|selected| selected != kind) {
            continue;
        }
        match place::suggest(client, &query, kind).await {
            Ok(suggestions) => {
                successful = true;
                places.extend(suggestions.places);
                for (scope, error) in suggestions.errors {
                    result.fail(scope, error);
                }
            }
            Err(error) if options.kind.is_some() => return Err(error),
            Err(error) => result.fail(format!("search.{}", kind.as_str()), error),
        }
    }
    if successful {
        places.sort_by(|a, b| {
            (
                api::normalize(&a.name) != query,
                &a.name,
                &a.kind,
                &a.qualified_name,
            )
                .cmp(&(
                    api::normalize(&b.name) != query,
                    &b.name,
                    &b.kind,
                    &b.qualified_name,
                ))
        });
        let total = places.len();
        places.truncate(options.limit as usize);
        result.data = Data::Search(SearchResult {
            query,
            total,
            results: places,
        });
    }
    Ok(result)
}

pub async fn station(client: &ApiClient, input: &str) -> Result<ResultData, Error> {
    let input = place::checked_input(input, Some(PlaceType::Subway))?;
    let (name, _) = place::split_name(&input);
    let stations = place::stations(client).await?;
    let station = &stations[api::choose(name, &stations, |s| s.name.clone())?];
    let mut result = ResultData::new(Data::Empty);
    let coords = match place::coordinates(client)
        .await
        .and_then(|records| place::station_coordinates(records, &station.name))
    {
        Ok(coords) => coords,
        Err(error) => {
            result.fail("station.coordinates", error);
            None
        }
    };
    result.data = Data::Station(StationInfo {
        name: station.name.clone(),
        qualified_name: place::qualified_name(&station.name, PlaceType::Subway),
        codes: station.codes.clone(),
        latitude: coords.map(|c| c.0),
        longitude: coords.map(|c| c.1),
    });
    Ok(result)
}
