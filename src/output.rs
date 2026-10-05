use crate::model::{Data, ResultData};
pub fn human(result: &ResultData, details: bool, quiet: bool) -> String {
    let mut out = String::new();
    use std::fmt::Write;
    match &result.data {
        Data::Empty => {}
        Data::Location(place) => {
            let _ = writeln!(
                out,
                "{} [{}]\n緯度: {}\n経度: {}",
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
            if data.places.is_empty() {
                let _ = writeln!(out, "該当する周辺の交通施設はありません");
            }
            for (i, place) in data.places.iter().enumerate() {
                let _ = writeln!(
                    out,
                    "{}. {} [{}]  {}m",
                    i + 1,
                    place.location.name,
                    kind_label(&place.location.kind),
                    place.distance_m
                );
            }
        }
        Data::Search(data) => {
            let _ = writeln!(out, "{}", data.query);
            if data.results.is_empty() {
                let _ = writeln!(out, "該当する駅・バス停はありません");
            }
            for (i, place) in data.results.iter().enumerate() {
                let _ = writeln!(
                    out,
                    "\n{}. {} [{}]",
                    i + 1,
                    place.name,
                    kind_label(&place.kind)
                );
                let info: Vec<_> = [&place.codes, &place.lines]
                    .into_iter()
                    .filter(|v| !v.is_empty())
                    .map(|v| v.join(" / "))
                    .collect();
                if !info.is_empty() {
                    let _ = writeln!(out, "   {}", info.join(" / "));
                }
                let _ = writeln!(out, "   {}", place.qualified_name);
            }
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
