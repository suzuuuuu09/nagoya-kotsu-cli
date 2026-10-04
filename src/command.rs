use crate::{
    api,
    cli::{Command, Status},
    client::ApiClient,
    error::Error,
    model::{Data, ResultData},
};
pub mod bus;
pub mod route;
pub mod subway;

pub async fn run(client: &ApiClient, command: &Command) -> Result<ResultData, Error> {
    match command {
        Command::Status(options) => status(client, options).await,
        Command::Bus { command } => bus::run(client, command).await,
        Command::Subway { command } => subway::run(client, command).await,
        Command::Route(options) => route::run(client, options).await,
    }
}
async fn status(client: &ApiClient, options: &Status) -> Result<ResultData, Error> {
    let mut status = api::status(client).await?;
    if let Some(line) = &options.line {
        let lines: Vec<_> = status
            .records
            .iter()
            .map(|r| r.line.clone())
            .chain(
                ["H_LINE", "M_LINE", "T_LINE", "S_LINE", "K_LINE", "B_LINE"]
                    .map(|id| api::line_name(id).to_owned()),
            )
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        let i = api::choose(line, &lines, Clone::clone)?;
        status.records.retain(|r| r.line == lines[i]);
    }
    if options.bus != options.subway {
        status
            .records
            .retain(|r| (r.line == "市バス") == options.bus);
    }
    Ok(ResultData::new(Data::Status(status)))
}
