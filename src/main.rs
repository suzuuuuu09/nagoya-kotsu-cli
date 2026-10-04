mod api;
mod cli;
mod client;
mod command;
mod error;
mod model;
mod output;
mod time;
use clap::Parser;

#[tokio::main]
async fn main() -> std::process::ExitCode {
    let cli = cli::Cli::parse();
    let client = match client::ApiClient::new(&cli) {
        Ok(c) => c,
        Err(e) => {
            report(&e, cli.verbose);
            return std::process::ExitCode::from(e.exit_code());
        }
    };
    match command::run(&client, &cli.command).await {
        Ok(result) => {
            let serialized = if cli.raw {
                serde_json::to_string_pretty(&client.raw().await)
            } else if cli.json {
                serde_json::to_string_pretty(&result)
            } else {
                Ok(output::human(
                    &result,
                    matches!(&cli.command, cli::Command::Route(r) if r.details),
                ))
            };
            match serialized {
                Ok(text) => println!("{text}"),
                Err(e) => {
                    eprintln!("error: 出力を生成できませんでした: {e}");
                    return std::process::ExitCode::from(1);
                }
            }
            for failure in &result.errors {
                eprintln!("warning: {}: {}", failure.scope, failure.message);
                if cli.verbose {
                    eprintln!("{}", failure.detail);
                }
            }
            std::process::ExitCode::from(result.exit_code())
        }
        Err(e) => {
            if cli.raw {
                match serde_json::to_string_pretty(&client.raw().await) {
                    Ok(raw) => println!("{raw}"),
                    Err(error) => eprintln!("error: raw出力を生成できませんでした: {error}"),
                }
            }
            report(&e, cli.verbose);
            std::process::ExitCode::from(e.exit_code())
        }
    }
}
fn report(error: &error::Error, verbose: bool) {
    eprintln!("error: {error}");
    if verbose {
        eprintln!("{error:?}");
    } else if matches!(
        error,
        error::Error::Parse(_)
            | error::Error::InvalidResponse(_)
            | error::Error::Network(_)
            | error::Error::Cache(_)
    ) {
        eprintln!("hint: --verbose で詳細を確認できます");
    }
}
