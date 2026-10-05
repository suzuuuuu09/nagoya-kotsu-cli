use crate::{
    api::{self, teiki},
    cli::Pass,
    client::ApiClient,
    error::Error,
    model::{Data, PassPrice, PassResult, PassRoute, PassType, ResultData},
};
use serde::Deserialize;
use serde_json::value::RawValue;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Deserialize)]
struct Class {
    #[serde(rename = "TEIKI")]
    class: teiki::Identifier,
}

fn prices(data: BTreeMap<String, Box<RawValue>>) -> (Vec<PassType>, Vec<(String, Error)>) {
    let mut types = Vec::new();
    let mut errors = Vec::new();
    for (name, body) in data {
        let mut prices = Vec::new();
        match serde_json::from_str::<BTreeMap<String, serde_json::Value>>(body.get()) {
            Ok(periods) => {
                for months in [1, 3, 6] {
                    let yen = periods
                        .get(&months.to_string())
                        .and_then(serde_json::Value::as_u64)
                        .and_then(|n| u32::try_from(n).ok());
                    if let Some(yen) = yen {
                        prices.push(PassPrice { months, yen });
                    } else {
                        errors.push((
                            format!("types.{name}.prices.{months}"),
                            Error::InvalidResponse(format!(
                                "{name}の{months}か月料金が欠落・不正です"
                            )),
                        ));
                    }
                }
            }
            Err(e) => errors.push((format!("types.{name}.prices"), Error::Parse(e.to_string()))),
        }
        types.push(PassType { name, prices });
    }
    (types, errors)
}

pub async fn run(client: &ApiClient, options: &Pass) -> Result<ResultData, Error> {
    let data = teiki::get_routes(
        client,
        &options.fare.from,
        &options.fare.to,
        options.fare.route.as_deref(),
    )
    .await?;
    let mut result = ResultData::new(Data::Empty);
    let mut candidates_complete = true;
    let mut routes = Vec::new();
    let mut classes = BTreeMap::new();
    let bus = if options.with_bus && !data.routes.is_empty() {
        Some(teiki::get_bus_pass_classes(client).await)
    } else {
        None
    };
    for (name, body) in data.routes {
        let class = serde_json::from_str::<Class>(body.get())
            .map_err(|e| Error::Parse(e.to_string()))
            .and_then(|c| teiki::checked_id(&c.class.0).map(str::to_owned));
        let class = class.and_then(|class| match &bus {
            Some(Ok(mapping)) => mapping
                .get(&class)
                .ok_or_else(|| Error::NotFound {
                    name: format!("{name}の市バス併用区分"),
                    candidates: String::new(),
                })
                .and_then(|id| teiki::checked_id(&id.0).map(str::to_owned)),
            Some(Err(e)) => Err(e.clone()),
            None => Ok(class),
        });
        match class {
            Ok(class) => {
                classes.insert(name.clone(), class);
            }
            Err(e) => {
                candidates_complete = false;
                result.fail(format!("pass.routes.{name}.prices"), e);
            }
        }
        routes.push(PassRoute {
            name,
            types: Vec::new(),
        });
    }
    let mut tasks = tokio::task::JoinSet::new();
    for class in classes.values().cloned().collect::<BTreeSet<_>>() {
        let client = client.clone();
        tasks.spawn(async move {
            let data = teiki::get_pass_prices(&client, &class).await.map(prices);
            (class, data)
        });
    }
    let mut fetched = BTreeMap::new();
    while let Some(item) = tasks.join_next().await {
        let (class, data) = item.map_err(|e| Error::Network(e.to_string()))?;
        fetched.insert(class, data);
    }
    for route in &mut routes {
        if let Some(class) = classes.get(&route.name) {
            match &fetched[class] {
                Ok((types, errors)) => {
                    route.types = types.clone();
                    for (scope, e) in errors {
                        result.fail(format!("pass.routes.{}.{}", route.name, scope), e.clone());
                    }
                }
                Err(e) => {
                    candidates_complete = false;
                    result.fail(format!("pass.routes.{}.prices", route.name), e.clone());
                }
            }
        }
    }
    if let Some(input) = &options.ticket_type {
        let names: Vec<_> = routes
            .iter()
            .flat_map(|r| r.types.iter().map(|t| t.name.clone()))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .filter(|name| candidates_complete || api::normalize(name) == api::normalize(input))
            .collect();
        let selected = if names.is_empty() && !candidates_complete {
            None
        } else {
            match api::choose(input, &names, Clone::clone) {
                Ok(i) => Some(names[i].clone()),
                Err(e) if candidates_complete => return Err(e),
                Err(e) => {
                    result.fail("pass.type", e);
                    None
                }
            }
        };
        for route in &mut routes {
            route.types.retain(|t| selected.as_ref() == Some(&t.name));
        }
    }
    if let Some(months) = &options.months {
        for route in &mut routes {
            for ticket in &mut route.types {
                ticket.prices.retain(|p| p.months.to_string() == *months);
            }
        }
    }
    result.errors.sort_by(|a, b| a.scope.cmp(&b.scope));
    result.data = Data::Pass(PassResult {
        from: data.from,
        to: data.to,
        routes,
    });
    Ok(result)
}
