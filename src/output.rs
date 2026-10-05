use crate::{
    cli::{Cli, Command},
    model::{Data, ResultData},
};
pub fn human(result: &ResultData, cli: &Cli, args: &[String]) -> String {
    let details = matches!(&cli.command, Command::Route(r) if r.details);
    let quiet = cli.quiet;
    let mut out = String::new();
    use std::fmt::Write;
    if !result.complete
        && matches!(
            &cli.command,
            Command::Search(_)
                | Command::Coordinates(_)
                | Command::Nearby(_)
                | Command::Subway {
                    command: crate::cli::Subway::Station { .. }
                }
        )
    {
        let scopes = result
            .errors
            .iter()
            .map(|e| e.scope.as_str())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
            .join("、");
        let status = if matches!(result.data, Data::Empty) {
            "取得失敗"
        } else {
            "部分結果"
        };
        let note = if matches!(result.data, Data::Empty) {
            "結果を取得できませんでした。"
        } else {
            "以下は取得できた範囲の情報です。"
        };
        let _ = writeln!(
            out,
            "{status}（終了コード{}）: 不足・不正な範囲: {scopes}。{note}",
            result.exit_code()
        );
    }
    match &result.data {
        Data::Empty => {}
        Data::Coordinates(place) => {
            let _ = writeln!(
                out,
                "{} [{}] の代表座標\n緯度: {}\n経度: {}",
                place.name,
                kind_label(&place.kind),
                place.latitude,
                place.longitude
            );
        }
        Data::Nearby(data) => {
            let _ = writeln!(
                out,
                "{} [{}] 周辺",
                data.origin.name,
                kind_label(&data.origin.kind)
            );
            if let Command::Nearby(options) = &cli.command {
                let radius = options.effective_radius();
                let range = radius
                    .map(|r| format!("半径: {r}m"))
                    .unwrap_or("距離制限なし・近い順".into());
                let _ = writeln!(
                    out,
                    "{range} / 探す施設: {} / 上限: {}件",
                    options
                        .kind
                        .map(|k| kind_label(k.as_str()))
                        .unwrap_or("市バス・地下鉄"),
                    options.limit
                );
                let _ = writeln!(
                    out,
                    "候補{}件・表示{}件{}",
                    data.matched_count,
                    data.places.len(),
                    if result.complete {
                        ""
                    } else {
                        "（取得できた範囲）"
                    }
                );
                if data.places.is_empty() && result.complete {
                    let _ = writeln!(out, "該当する周辺の交通施設はありません");
                    if radius.is_some() {
                        let mut retry = args.to_vec();
                        replace_place(&mut retry, "", &data.origin.qualified_name);
                        set_option(&mut retry, "--radius", None);
                        set_option(&mut retry, "--no-radius", Some(""));
                        let _ =
                            writeln!(out, "距離制限を外して再検索:\n  {}", command_text(&retry));
                    }
                    if radius.is_none() && options.kind.is_none() {
                        let _ = writeln!(out, "取得した対象データの範囲で0件です。");
                    }
                    if options.kind.is_some() {
                        let mut retry = args.to_vec();
                        replace_place(&mut retry, "", &data.origin.qualified_name);
                        set_option(&mut retry, "--result-type", None);
                        let _ = writeln!(
                            out,
                            "結果種別を両方にして再検索:\n  {}",
                            command_text(&retry)
                        );
                    }
                } else if data.places.is_empty() {
                    let _ = writeln!(
                        out,
                        "取得できた範囲に表示できる候補はありません。全範囲の該当なしとは断定できません。"
                    );
                }
                if data.matched_count > data.places.len() {
                    let mut retry = args.to_vec();
                    replace_place(&mut retry, "", &data.origin.qualified_name);
                    set_option(&mut retry, "--limit", Some(&data.matched_count.to_string()));
                    let _ = writeln!(out, "表示件数を増やす:\n  {}", command_text(&retry));
                }
            }
            let _ = writeln!(
                out,
                "距離は代表点間の概算直線距離です。徒歩距離・徒歩時間、入口・のりば間の距離ではありません。"
            );
            for (i, place) in data.places.iter().enumerate() {
                let _ = writeln!(
                    out,
                    "{}. {} [{}]  {}m\n   入力用: {}",
                    i + 1,
                    place.location.name,
                    kind_label(&place.location.kind),
                    place.distance_m,
                    shell_word(&place.location.qualified_name)
                );
            }
        }
        Data::Search(data) => {
            let _ = writeln!(
                out,
                "{}\n取得済み候補{}件・表示{}件{}",
                data.query,
                data.retrieved_count,
                data.results.len(),
                if result.complete {
                    ""
                } else {
                    "（取得できた範囲）"
                }
            );
            if data.results.is_empty() {
                let _ = writeln!(
                    out,
                    "{}",
                    if result.complete {
                        "該当する駅・バス停はありません"
                    } else {
                        "取得できた範囲に表示できる候補はありません。全範囲の該当なしとは断定できません。"
                    }
                );
            }
            for (i, place) in data.results.iter().enumerate() {
                let _ = writeln!(
                    out,
                    "\n{}. {} [{}]",
                    i + 1,
                    place.name,
                    kind_label(&place.kind)
                );
                if !place.codes.is_empty() {
                    let _ = writeln!(out, "   駅記号: {}", place.codes.join(" / "));
                }
                if !place.reported_lines.is_empty() {
                    let _ = writeln!(
                        out,
                        "   参考路線・系統: {}",
                        place.reported_lines.join(" / ")
                    );
                }
                let _ = writeln!(out, "   入力用: {}", shell_word(&place.qualified_name));
            }
            let _ = writeln!(
                out,
                "参考路線・系統は取得できた情報のみで、所属路線を網羅しません。座標は coordinates で確認できます。"
            );
        }
        Data::Station(station) => {
            let latitude = station
                .latitude
                .map(|v| v.to_string())
                .unwrap_or("不明".into());
            let longitude = station
                .longitude
                .map(|v| v.to_string())
                .unwrap_or("不明".into());
            let _ = writeln!(
                out,
                "{}\n駅記号: {}\n緯度: {}\n経度: {}",
                station.name,
                station.codes.join(" / "),
                latitude,
                longitude
            );
        }
        Data::Fare(fare) => {
            let _ = writeln!(out, "{} → {}", fare.from, fare.to);
            if fare.routes.is_empty() {
                let _ = writeln!(out, "該当する運賃経路はありません");
            }
            for (i, route) in fare.routes.iter().enumerate() {
                let yen = route
                    .fare_yen
                    .map(|v| format!("{v}円"))
                    .unwrap_or("不明".into());
                let _ = writeln!(out, "\n{}. {}\n   普通運賃: {yen}", i + 1, route.name);
            }
        }
        Data::Pass(pass) => {
            let _ = writeln!(out, "{} → {}", pass.from, pass.to);
            if pass.routes.is_empty() {
                let _ = writeln!(out, "該当する定期券の料金経路はありません");
            }
            for route in &pass.routes {
                let _ = writeln!(out, "\n{}", route.name);
                if route.types.is_empty() {
                    let _ = writeln!(out, "  表示できる券種・料金はありません");
                }
                for ticket in &route.types {
                    let _ = writeln!(out, "\n{}", ticket.name);
                    for price in &ticket.prices {
                        let _ = writeln!(out, "  {}か月  {}円", price.months, price.yen);
                    }
                }
            }
            if !quiet {
                let _ = writeln!(
                    out,
                    "\n※表示料金は料金データに基づくもので、購入資格を保証しません。"
                );
            }
        }
        Data::DelayCertificates(data) => {
            if data.certificates.is_empty() {
                let _ = writeln!(out, "該当する延着証明書はありません");
            }
            for (i, cert) in data.certificates.iter().enumerate() {
                let _ = writeln!(
                    out,
                    "\n{}. {}  {}\n   {}\n   最大遅延: {}\n   証明書: {}",
                    i + 1,
                    cert.date,
                    cert.line,
                    cert.title,
                    cert.max_delay_time,
                    cert.url
                );
            }
        }
        Data::Documents(documents) => {
            let width = documents
                .iter()
                .map(|document| document.name.len())
                .max()
                .unwrap_or(0);

            for document in documents {
                let _ = writeln!(
                    out,
                    "{:<width$}  {}",
                    document.name,
                    document.summary,
                    width = width
                );
            }
        }
        Data::Document(document) => return document.content.unwrap_or_default().to_owned(),
        Data::Status(status) => {
            for r in &status.records {
                let _ = writeln!(out, "{}\n{}", r.line, r.title);
                if !r.message.is_empty() {
                    let _ = writeln!(out, "{}", r.message);
                }
                if let Some(date) = &r.created_at {
                    let _ = writeln!(out, "記事作成日時: {date}");
                }
            }
            let _ = writeln!(out, "API取得日時: {}", status.fetched_at);
        }
        Data::Stop(stop) => {
            let _ = writeln!(out, "{} [{}]", stop.name, stop.id);
            for pole in &stop.poles {
                let _ = writeln!(out, "\n{}", pole.name);
                if let Some(lat) = pole.latitude {
                    let _ = writeln!(out, "  緯度: {lat}");
                }
                if let Some(lng) = pole.longitude {
                    let _ = writeln!(out, "  経度: {lng}");
                }
            }
        }
        Data::Timetable(t) => {
            let _ = writeln!(
                out,
                "{} {}\n営業日: {}\n日種: {}\n予定時刻",
                t.station.name,
                t.station.codes.join("/"),
                t.service_date,
                t.day_type
            );
            if t.departures.is_empty() {
                let _ = writeln!(out, "該当する予定便はありません");
            }
            for d in &t.departures {
                let _ = writeln!(
                    out,
                    "{} {} {} {} {} ({}, {})",
                    d.time,
                    d.line,
                    d.direction,
                    d.platform,
                    d.destination.as_deref().unwrap_or("行先不明"),
                    d.service_date,
                    d.day_type
                );
            }
        }
        Data::Live(live) => {
            let _ = writeln!(out, "{}\nAPI取得日時: {}", live.stop, live.fetched_at);
            if !live.notice.is_empty() {
                let _ = writeln!(out, "{}", live.notice);
            }
            if live.service_active == Some(false) {
                let _ = writeln!(out, "接近情報サービス停止中");
            }
            let mut count = 0;
            for route in &live.routes {
                let _ = writeln!(
                    out,
                    "\n{} {} {}行 {}",
                    route.name,
                    route.via,
                    route.destination,
                    route.poles.join("、")
                );
                for bus in &route.vehicles {
                    count += 1;
                    let _ = writeln!(out, "  {}", bus.vehicle);
                    if let Some(p) = &bus.current_position {
                        let _ = writeln!(
                            out,
                            "  {} → {} ({})",
                            p.from_stop.as_deref().unwrap_or("不明"),
                            p.to_stop.as_deref().unwrap_or("不明"),
                            match p.relation.as_str() {
                                "approaching" => "未通過",
                                "passed" => "通過済み",
                                _ => "位置関係不明",
                            }
                        );
                    } else {
                        let _ = writeln!(out, "  現在位置情報なし");
                    }
                    if let Some(p) = &bus.latest_pass {
                        let _ = writeln!(out, "  最終通過: {} {}", p.stop, p.time);
                    }
                }
            }
            if count == 0 {
                let _ = writeln!(out, "対象のバス情報はありません");
            }
        }
        Data::Route(r) => {
            if let Command::Route(options) = &cli.command {
                let mode = if options.bus != options.subway {
                    if options.bus {
                        "市バスのみ"
                    } else {
                        "地下鉄のみ"
                    }
                } else {
                    "市バス・地下鉄"
                };
                let _ = writeln!(out, "検索対象の交通手段: {mode}");
            }
            let _ = writeln!(out, "{} → {}", r.from.name, r.to.name);
            if r.routes.is_empty() {
                let _ = writeln!(out, "該当する経路はありません");
            }
            for (i, route) in r.routes.iter().enumerate() {
                let _ = writeln!(
                    out,
                    "\n{}. {} → {}\n{}分 / {}円 / 乗換{}回",
                    i + 1,
                    route.departure,
                    route.arrival,
                    route
                        .duration_minutes
                        .map(|v| v.to_string())
                        .unwrap_or("不明".into()),
                    route
                        .fare_yen
                        .map(|v| v.to_string())
                        .unwrap_or("不明".into()),
                    route.transfers
                );
                for s in &route.segments {
                    let _ = writeln!(
                        out,
                        "  {} {}\n  {} {} → {} {}",
                        s.line, s.direction, s.from, s.departure, s.to, s.arrival
                    );
                    if details {
                        let _ = writeln!(
                            out,
                            "  {} → {} / {}円",
                            s.from_platform,
                            s.to_platform,
                            s.fare_yen.map(|v| v.to_string()).unwrap_or("不明".into())
                        );
                    }
                }
            }
        }
    }
    out
}

fn kind_label(kind: &str) -> &str {
    match kind {
        "bus" => "市バス",
        "subway" => "地下鉄",
        _ => kind,
    }
}

pub fn shell_word(value: &str) -> String {
    if !value.is_empty()
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_./:=,".contains(&b))
    {
        value.into()
    } else {
        format!("'{}'", value.replace('\'', "'\"'\"'"))
    }
}

fn command_text(args: &[String]) -> String {
    format!(
        "nkotsu {}",
        args.iter()
            .map(|s| shell_word(s))
            .collect::<Vec<_>>()
            .join(" ")
    )
}

fn value_options() -> Vec<String> {
    use clap::CommandFactory;
    fn collect(command: &clap::Command, out: &mut Vec<String>) {
        for arg in command
            .get_arguments()
            .filter(|a| a.get_action().takes_values())
        {
            if let Some(long) = arg.get_long() {
                out.push(format!("--{long}"));
            }
            if let Some(short) = arg.get_short() {
                out.push(format!("-{short}"));
            }
        }
        for command in command.get_subcommands() {
            collect(command, out);
        }
    }
    let mut out = Vec::new();
    collect(&crate::cli::Cli::command(), &mut out);
    out
}

fn positional_indices(args: &[String]) -> Vec<usize> {
    let options = value_options();
    let mut indices = Vec::new();
    let mut i = 0;
    let mut literal = false;
    while i < args.len() {
        if !literal && args[i] == "--" {
            literal = true;
        } else if !literal && options.contains(&args[i]) {
            i += 1;
        } else if literal || !args[i].starts_with('-') {
            indices.push(i);
        }
        i += 1;
    }
    indices
}

fn set_option(args: &mut Vec<String>, option: &str, value: Option<&str>) {
    let end = args.iter().position(|a| a == "--").unwrap_or(args.len());
    if let Some(i) = args[..end]
        .iter()
        .position(|a| a == option || a.starts_with(&format!("{option}=")))
    {
        let count = if args[i] == option && value_options().contains(&args[i]) {
            2
        } else {
            1
        };
        args.drain(i..i + count);
    }
    if let Some(value) = value {
        let i = args.iter().position(|a| a == "--").unwrap_or(args.len());
        if value_options().iter().any(|o| o == option) && value.starts_with('-') {
            args.insert(i, format!("{option}={value}"));
        } else {
            args.insert(i, option.into());
            if value_options().iter().any(|o| o == option) {
                args.insert(i + 1, value.into());
            }
        }
    }
}

fn replace_place(args: &mut Vec<String>, scope: &str, value: &str) {
    if scope.ends_with(".via") {
        set_option(args, "--via", Some(value));
        return;
    }
    let indices = positional_indices(args);
    let depth = indices
        .first()
        .map(|&i| usize::from(matches!(args[i].as_str(), "bus" | "subway")) + 1)
        .unwrap_or(1);
    let offset = usize::from(scope.ends_with(".to"));
    if let Some(&i) = indices.get(depth + offset) {
        args[i] = value.into();
        if value.starts_with('-')
            && args
                .iter()
                .position(|a| a == "--")
                .is_none_or(|end| i < end)
        {
            let places: Vec<_> = indices[depth..].iter().map(|&i| args[i].clone()).collect();
            for &i in indices[depth..].iter().rev() {
                args.remove(i);
            }
            if let Some(i) = args.iter().position(|a| a == "--") {
                args.remove(i);
            }
            args.push("--".into());
            args.extend(places);
        }
    }
}

pub fn recovery(error: &crate::error::Error, args: &[String]) -> String {
    use crate::{api, error::Error};
    use std::fmt::Write;
    let mut out = String::new();
    let scope = match error {
        Error::Scoped { scope, .. } => scope.as_str(),
        _ => "",
    };
    let role = match scope.rsplit('.').next() {
        Some("from") => Some("出発地（from）"),
        Some("to") => Some("到着地（to）"),
        Some("via") => Some("経由地（--via）"),
        _ => None,
    };
    if let Some(role) = role {
        let _ = writeln!(out, "対象の地点引数: {role}");
    }
    match error.source_error() {
        Error::PlaceAmbiguous { candidates, .. } => {
            for candidate in candidates {
                let _ = writeln!(
                    out,
                    "候補: {} [{}]",
                    candidate.name,
                    kind_label(&candidate.kind)
                );
                // ponytail: O(n²) collision check; pre-count names if large masters make it slow.
                if candidates
                    .iter()
                    .filter(|c| {
                        api::normalize(&c.qualified_name)
                            == api::normalize(&candidate.qualified_name)
                    })
                    .count()
                    > 1
                {
                    let _ = writeln!(
                        out,
                        "  データ上の候補を完全修飾名でも区別できません。自動選択せず、データの修正を待つ必要があります。"
                    );
                } else {
                    let mut retry = args.to_vec();
                    replace_place(&mut retry, scope, &candidate.qualified_name);
                    let _ = writeln!(out, "  {}", command_text(&retry));
                }
            }
        }
        Error::PlaceNotFound { name, kind } => {
            let normalized = api::normalize(name);
            let (name, suffix) = api::place::split_name(&normalized);
            let mut search = vec!["search".into(), name.into()];
            if let Some(kind) = kind.or(suffix) {
                search.extend(["--type".into(), kind.as_str().into()]);
            }
            if name.starts_with('-') {
                search.remove(1);
                search.extend(["--".into(), name.into()]);
            }
            let _ = writeln!(
                out,
                "名前を検索して確認できます（候補が見つかるとは限りません）:\n  {}",
                command_text(&search)
            );
        }
        Error::CoordinatesUnavailable { name, kind, .. } => {
            let literal = if name.starts_with('-') { "-- " } else { "" };
            let command = if *kind == crate::cli::PlaceType::Bus {
                "bus stop"
            } else {
                "subway station"
            };
            let _ = writeln!(
                out,
                "基本情報を確認できます（座標の問題は解消しません）:\n  nkotsu {command} {literal}{}",
                shell_word(name)
            );
        }
        _ => {}
    }
    out
}
