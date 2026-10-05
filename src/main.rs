mod api;
mod cli;
mod client;
mod command;
mod docs;
mod error;
mod model;
mod output;
mod time;
use clap::Parser;

#[tokio::main]
async fn main() -> std::process::ExitCode {
    let args: Vec<_> = std::env::args_os().collect();
    let cli = match cli::Cli::try_parse_from(&args) {
        Ok(cli) => cli,
        Err(error) => {
            let json = args
                .iter()
                .skip(1)
                .take_while(|arg| *arg != "--")
                .any(|arg| arg == "--json");
            if error.exit_code() != 0 && json {
                failure(&error::Error::Arguments(error.to_string()), true, false);
                return std::process::ExitCode::from(2);
            }
            error.exit();
        }
    };
    if cli.json && cli.raw {
        failure(
            &error::Error::Arguments("--json と --raw は同時指定できません".into()),
            true,
            cli.verbose,
        );
        return std::process::ExitCode::from(2);
    }
    if let cli::Command::Docs { command } = &cli.command {
        let result = if cli.raw {
            Err(error::Error::Arguments(
                "docs では --raw を利用できません。--json を指定してください".into(),
            ))
        } else {
            docs::run(command)
        };
        return match result {
            Ok(result) => display(&cli, &result, None),
            Err(e) => {
                failure(&e, cli.json, cli.verbose);
                std::process::ExitCode::from(e.exit_code())
            }
        };
    }
    let client = match client::ApiClient::new(&cli) {
        Ok(c) => c,
        Err(e) => {
            failure(&e, cli.json, cli.verbose);
            return std::process::ExitCode::from(e.exit_code());
        }
    };
    match command::run(&client, &cli.command).await {
        Ok(result) => {
            let raw = if cli.raw {
                Some(client.raw().await)
            } else {
                None
            };
            let exit = display(&cli, &result, raw.as_ref());
            for failure in &result.errors {
                eprintln!("warning: {}: {}", failure.scope, failure.message);
                if cli.verbose {
                    eprintln!("{}", failure.detail);
                }
            }
            exit
        }
        Err(e) => {
            if cli.raw {
                match serde_json::to_string_pretty(&client.raw().await) {
                    Ok(raw) => println!("{raw}"),
                    Err(error) => eprintln!("error: raw出力を生成できませんでした: {error}"),
                }
            }
            failure(&e, cli.json, cli.verbose);
            let args: Vec<String> = args
                .iter()
                .skip(1)
                .map(|a| a.to_string_lossy().into_owned())
                .collect();
            let recovery = output::recovery(&e, &args);
            if !recovery.is_empty() {
                eprint!("{recovery}");
            }
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

fn failure(error: &error::Error, json: bool, verbose: bool) {
    if json {
        let result = model::ResultData {
            schema_version: 1,
            complete: false,
            data: model::Data::Empty,
            errors: vec![error::Failure::new("command", error.clone())],
        };
        match serde_json::to_string_pretty(&result) {
            Ok(text) => println!("{text}"),
            Err(e) => eprintln!("error: 出力を生成できませんでした: {e}"),
        }
    }
    report(error, verbose);
}

fn display(
    cli: &cli::Cli,
    result: &model::ResultData,
    raw: Option<&std::collections::BTreeMap<String, String>>,
) -> std::process::ExitCode {
    let serialized = if let Some(raw) = raw {
        serde_json::to_string_pretty(raw)
    } else if cli.json {
        serde_json::to_string_pretty(result)
    } else {
        let args: Vec<String> = std::env::args_os()
            .skip(1)
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        Ok(output::human(result, cli, &args))
    };
    match serialized {
        Ok(text) => {
            println!("{text}");
            std::process::ExitCode::from(result.exit_code())
        }
        Err(e) => {
            eprintln!("error: 出力を生成できませんでした: {e}");
            std::process::ExitCode::from(1)
        }
    }
}
