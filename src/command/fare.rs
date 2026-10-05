use crate::{
    api::teiki,
    cli::Fare,
    client::ApiClient,
    error::Error,
    model::{Data, FareResult, FareRoute, ResultData},
};
use serde::Deserialize;

#[derive(Deserialize)]
struct Price {
    #[serde(rename = "FARE")]
    yen: u32,
}

pub async fn run(client: &ApiClient, options: &Fare) -> Result<ResultData, Error> {
    let data =
        teiki::get_routes(client, &options.from, &options.to, options.route.as_deref()).await?;
    let mut result = ResultData::new(Data::Empty);
    let mut routes = Vec::new();
    for (name, body) in data.routes {
        let fare_yen = match serde_json::from_str::<Price>(body.get()) {
            Ok(price) => Some(price.yen),
            Err(e) => {
                result.fail(
                    format!("fare.routes.{name}.fare_yen"),
                    Error::Parse(e.to_string()),
                );
                None
            }
        };
        routes.push(FareRoute { name, fare_yen });
    }
    result.data = Data::Fare(FareResult {
        from: data.from,
        to: data.to,
        routes,
    });
    Ok(result)
}
