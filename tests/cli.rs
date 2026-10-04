use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command;

#[test]
fn status_keeps_duplicate_line_articles_and_accepts_bom() {
    let server = Server::new(|_| {
        Response::json(
            "\u{feff}[{\"rosen_id\":\"H_LINE\",\"traffic_title\":\"平常運行\",\"unknown\":true},{\"rosen_id\":\"H_LINE\",\"traffic_title\":\"運行変更\"}]",
        )
    });
    let output = server.run(&["--no-cache", "--json", "status"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["schema_version"], 1);
    assert_eq!(value["complete"], true);
    assert_eq!(value["data"]["records"].as_array().unwrap().len(), 2);
    assert_eq!(value["data"]["records"][1]["title"], "運行変更");
}

#[test]
fn bus_stop_preserves_string_ids_and_reuses_persistent_master_cache() {
    let server = Server::new(|request| {
        let body = if request.contains("station_name.json") {
            r#"{"上社":"0011180"}"#
        } else {
            r#"{"POLES":{"002":{"lat":35.17,"lng":137.0,"name":"4番"}}}"#
        };
        Response::json(body).header("Cache-Control: max-age=3600\r\n")
    });
    let cache = std::env::temp_dir().join(format!("nkotsu-test-cache-{}", std::process::id()));
    for _ in 0..2 {
        let output = Command::new(env!("CARGO_BIN_EXE_nkotsu"))
            .env("NKOTSU_BASE_URL", &server.url)
            .env("NKOTSU_CACHE_DIR", &cache)
            .args(["--json", "bus", "stop", " 上社 "])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["data"]["id"], "0011180");
        assert_eq!(value["data"]["poles"][0]["name"], "4番");
    }
    assert_eq!(server.requests.lock().unwrap().len(), 2);
    if let Ok(files) = std::fs::read_dir(&cache) {
        for file in files {
            std::fs::remove_file(file.unwrap().path()).unwrap();
        }
        std::fs::remove_dir(&cache).unwrap();
    }
}

struct Response {
    status: u16,
    mime: &'static str,
    body: String,
    headers: String,
}
impl Response {
    fn json(body: impl Into<String>) -> Self {
        Self {
            status: 200,
            mime: "application/json",
            body: body.into(),
            headers: String::new(),
        }
    }
    fn header(mut self, headers: &str) -> Self {
        self.headers = headers.into();
        self
    }
}
struct Server {
    url: String,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    requests: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
}
impl Server {
    fn new(handler: impl Fn(&str) -> Response + Send + 'static) -> Self {
        use std::sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        listener.set_nonblocking(true).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let log = requests.clone();
        let thread = std::thread::spawn(move || {
            while !flag.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        stream
                            .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                            .unwrap();
                        let mut request = Vec::new();
                        let mut chunk = [0; 4096];
                        loop {
                            let n = stream.read(&mut chunk).unwrap_or(0);
                            if n == 0 {
                                break;
                            }
                            request.extend_from_slice(&chunk[..n]);
                            if request.ends_with(b"\r\n\r\n") {
                                break;
                            }
                        }
                        let request = String::from_utf8(request).unwrap();
                        let response = handler(&request);
                        log.lock().unwrap().push(request);
                        let _ = write!(
                            stream,
                            "HTTP/1.1 {} OK\r\nContent-Type: {}\r\nContent-Length: {}\r\n{}Connection: close\r\n\r\n{}",
                            response.status,
                            response.mime,
                            response.body.len(),
                            response.headers,
                            response.body
                        );
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(std::time::Duration::from_millis(5))
                    }
                    Err(e) => panic!("{e}"),
                }
            }
        });
        Self {
            url,
            stop,
            thread: Some(thread),
            requests,
        }
    }
    fn run(&self, args: &[&str]) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_nkotsu"))
            .env("NKOTSU_BASE_URL", &self.url)
            .args(args)
            .output()
            .unwrap()
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        self.thread.take().unwrap().join().unwrap();
    }
}

#[test]
fn subway_timetable_pairs_destinations_and_uses_service_hours() {
    let server = Server::new(|request| {
        Response::json(if request.contains("station_master.json") {
            r#"[{"id":"050","name":"藤が丘","codes":["H22"]}]"#
        } else {
            r#"{"東山線":[{"DIRECTION":"上り","RAILWAY":"高畑方面","PLATFORM":"2番線","DIAGRAM":{"平日":{"1":["03"],"12":[3,8]}},"DESTINATION":{"平日":{"1":["岩"],"12":["","岩"]}},"NOTES":{"平日":["岩…岩塚行"]}}]}"#
        })
    });
    let output = server.run(&[
        "--no-cache",
        "--json",
        "subway",
        "timetable",
        "藤が丘",
        "--day",
        "weekday",
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let d = v["data"]["departures"].as_array().unwrap();
    assert_eq!(d[0]["destination"], "高畑");
    assert_eq!(d[1]["destination"], "岩塚");
    assert_eq!(d[2]["time"], "25:03");
    assert_eq!(d[2]["destination"], "岩塚");
    assert!(server.requests.lock().unwrap()[1].contains("/diagrams/050.json"));
}

#[test]
fn bus_timetable_filters_nfkc_route_and_keeps_destination_symbols() {
    let server = Server::new(|request| {
        Response::json(if request.contains("station_name.json") {
            r#"{"上社":11180}"#
        } else if request.contains("/stations/") {
            r#"{"POLES":{"002":{"name":"4番"}}}"#
        } else {
            r#"{"上社１２":[{"POLE":"002","POLENAME":"4番","RAILWAY":["星ケ丘","緑ケ丘住宅"],"DIAGRAM":{"平日":{"12":[3,8]}},"DESTINATION":{"平日":{"12":["　","緑"]}},"NOTE":{"平日":["　星ケ丘","緑緑ケ丘住宅"]}}]}"#
        })
    });
    let out = server.run(&[
        "--no-cache",
        "--json",
        "bus",
        "timetable",
        "上社",
        "--day",
        "weekday",
        "--route",
        "上社12",
        "--pole",
        "4番",
        "--after",
        "12:04",
        "--limit",
        "1",
    ]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["data"]["departures"][0]["time"], "12:08");
    assert_eq!(v["data"]["departures"][0]["destination"], "緑ケ丘住宅");
}

#[test]
fn bus_live_deduplicates_routes_and_never_infers_position_from_history() {
    let server = Server::new(|request| {
        Response::json(if request.contains("station_name.json") {
            r#"{"上社":"11180","一社":"12180"}"#
        } else if request.contains("/stations/") {
            r#"{"POLES":{"002":{"name":"4番"},"003":{"name":"3番"}}}"#
        } else if request.contains("bus_service_status") {
            r#"[{"bus_service_active":true}]"#
        } else if request.contains("/busstops/") {
            r#"{"POLES":[{"CODE":"002","NORIBA":"4番","KEITOS":["001"]},{"CODE":"003","NORIBA":"3番","KEITOS":["001"]}]}"#
        } else if request.contains("/keitos/") {
            r#"{"NAME":"上社１２","TO":"星ケ丘","BUSSTOPS":["12180002","11180002"]}"#
        } else if request.contains("buspole_infos") {
            r#"{"12180002":{"BC":"002"},"11180002":{"BC":"002"}}"#
        } else {
            r#"{"LATEST_BUS_PASS":{"12180/002":{"NS 0391":"01:34:21"}},"unknown":true}"#
        })
    });
    let out = server.run(&[
        "--no-cache",
        "--json",
        "bus",
        "live",
        "上社",
        "--route",
        "上社12",
    ]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let route = &v["data"]["routes"][0];
    assert_eq!(route["poles"].as_array().unwrap().len(), 2);
    assert!(route["vehicles"][0]["current_position"].is_null());
    assert_eq!(route["vehicles"][0]["latest_pass"]["time"], "25:34:21");
    assert_eq!(route["vehicles"][0]["latest_pass"]["stop"], "一社");
    let requests = server.requests.lock().unwrap();
    assert_eq!(
        requests
            .iter()
            .filter(|r| r.contains("realtime_json"))
            .count(),
        1
    );
    assert!(
        requests
            .iter()
            .find(|r| r.contains("realtime_json"))
            .unwrap()
            .contains("?_=")
    );
}

#[test]
fn route_parses_namespaced_xml_and_inverts_arrival_option() {
    let server = Server::new(|request| {
        if request.contains("StationInfos") {
            let name = if request.contains("%E8%97%A4") {
                "藤が丘"
            } else {
                "名古屋"
            };
            Response::json(format!(
                r#"[{{"station_cd":"0001","station_div":"2","station_name":"{name}","latitude":35.0,"longitude":137.0}}]"#
            ))
        } else if request.contains("SETTING.js") {
            Response {
                status: 200,
                mime: "application/javascript",
                body: r#"var s={guid:"public-site-guid",apipath:"/NagoyaRoute/PRD/"};"#.into(),
                headers: String::new(),
            }
        } else {
            Response {status:200,mime:"text/xml",headers:String::new(),body:r#"<NorikaeData xmlns="urn:official"><Nstatus><Status>true</Status></Nstatus><NDS><HYOUKA><ROUTENO>001</ROUTENO><ROUTECOUNT>99</ROUTECOUNT><MINUTE>29</MINUTE><FARE>310</FARE><TIME_BEFORE_FIRST_WALK>2026/10/05 09:01:00</TIME_BEFORE_FIRST_WALK><TIME_AFTER_LAST_WALK>2026/10/05 09:30:00</TIME_AFTER_LAST_WALK></HYOUKA><ROUTE><ROUTENO>001</ROUTENO><LINENO>000</LINENO><ROSENNAME>東山線</ROSENNAME><FROMEKINAME>藤が丘(名古屋市地下鉄)</FROMEKINAME><TOEKINAME>名古屋(名古屋市地下鉄)</TOEKINAME><FROMDATETIME>2026/10/05 09:01:00</FROMDATETIME><TODATETIME>2026/10/05 09:30:00</TODATETIME><DIRECTIONNAME>高畑行</DIRECTIONNAME><KEIRONAME>バス</KEIRONAME><unknown/></ROUTE></NDS></NorikaeData>"#.into() }
        }
    });
    let out = server.run(&[
        "--no-cache",
        "--json",
        "route",
        "藤が丘",
        "名古屋",
        "--subway",
        "--at",
        "2026-10-05T09:00",
        "--arrive",
    ]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let route = &v["data"]["routes"][0];
    assert_eq!(route["departure"], "2026-10-05T09:01:00+09:00");
    assert_eq!(route["fare_yen"], 310);
    assert_eq!(route["riding_segments"], 1);
    assert_eq!(route["transfers"], 0);
    let requests = server.requests.lock().unwrap();
    let r = requests
        .iter()
        .find(|r| r.contains("SearchRouteDiagram"))
        .unwrap();
    assert!(r.contains("blnArrival=false"));
    assert!(r.contains("fromLatitude=126000000"));
    assert!(r.contains("intHour=9"));
}

#[test]
fn http_failures_retry_only_allowed_statuses_and_reject_html() {
    for (status, mime, body, code, requests) in [
        (400, "application/json", "{}", 4, 1),
        (502, "application/json", "{}", 4, 3),
        (503, "application/json", "{}", 4, 3),
        (504, "application/json", "{}", 4, 3),
        (200, "text/html", "<html>maintenance</html>", 5, 1),
        (200, "application/json", "<html>maintenance</html>", 5, 1),
        (200, "application/json", "{invalid", 5, 1),
    ] {
        let server = Server::new(move |_| Response {
            status,
            mime,
            body: body.into(),
            headers: String::new(),
        });
        let out = server.run(&["--no-cache", "status"]);
        assert_eq!(out.status.code(), Some(code));
        assert!(!String::from_utf8_lossy(&out.stderr).contains(&server.url));
        assert_eq!(server.requests.lock().unwrap().len(), requests);
    }
    let count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = count.clone();
    let server = Server::new(move |_| {
        let mut r = Response::json("[]");
        if counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed) == 0 {
            r.status = 502;
        }
        r
    });
    assert!(server.run(&["--no-cache", "status"]).status.success());
    assert_eq!(count.load(std::sync::atomic::Ordering::Relaxed), 2);
}

#[test]
fn ambiguous_endpoint_is_not_selected_and_raw_keeps_original_body() {
    let server = Server::new(|_| {
        Response::json(
            r#"[{"station_cd":"001","station_div":"1","station_name":"藤が丘","latitude":35.0,"longitude":137.0},{"station_cd":"002","station_div":"2","station_name":"藤が丘","latitude":35.0,"longitude":137.0}]"#,
        )
    });
    let out = server.run(&["--no-cache", "route", "藤が丘", "藤が丘"]);
    assert_eq!(out.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&out.stderr).contains("名古屋市地下鉄"));
    assert!(String::from_utf8_lossy(&out.stderr).contains("名古屋市バス"));
    assert_eq!(server.requests.lock().unwrap().len(), 1);
    let original = "\u{feff}[ {\"rosen_id\":\"H_LINE\",\"unknown\":true} ]\n";
    let server = Server::new(move |_| Response::json(original));
    let out = server.run(&["--no-cache", "--raw", "status"]);
    assert!(out.status.success());
    let raw: std::collections::BTreeMap<String, String> =
        serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(raw.values().next().unwrap(), original);
    let out = server.run(&["--json", "--raw", "status"]);
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn next_searches_next_service_day_and_keeps_initial_day_override_local() {
    let tomorrow = (chrono::Utc::now().with_timezone(&chrono_tz::Asia::Tokyo)
        - chrono::Duration::hours(4))
    .date_naive()
    .succ_opt()
    .unwrap();
    let settings = format!(
        r#"var s={{new_year_start:"2021/01/01 04:00:00",new_year_end:"2021/01/04 04:00:00",allnight_start:"2021/01/01 04:00:00",allnight_end:"2021/01/02 04:00:00",special_holiday_dates:["{}"]}};"#,
        tomorrow.format("%Y/%m/%d")
    );
    let server = Server::new(move |request| {
        if request.contains("station_master") {
            Response::json(r#"[{"id":"050","name":"藤が丘","codes":["H22"]}]"#)
        } else if request.contains("subway_setting") {
            Response {
                status: 200,
                mime: "application/javascript",
                headers: String::new(),
                body: settings.clone(),
            }
        } else {
            Response::json(
                r#"{"東山線":[{"RAILWAY":"高畑方面","DIAGRAM":{"平日":{"23":[59]},"土休日":{"5":[10,20,30]},"終夜":{"3":[55]}},"DESTINATION":null,"NOTES":null}]}"#,
            )
        }
    });
    let out = server.run(&[
        "--no-cache",
        "--json",
        "subway",
        "next",
        "藤が丘",
        "--at",
        "23:59",
        "--day",
        "weekday",
        "--limit",
        "3",
    ]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let d = v["data"]["departures"].as_array().unwrap();
    assert_eq!(d.len(), 3);
    assert_eq!(d[0]["time"], "23:59");
    assert_eq!(d[1]["time"], "05:10");
    assert_eq!(d[1]["day_type"], "土休日");
    assert_eq!(d[1]["service_date"], tomorrow.to_string());
    assert_eq!(v["data"]["scheduled"], true);
    assert_eq!(
        server
            .requests
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.contains("/diagrams/"))
            .count(),
        1
    );
    let out = server.run(&[
        "--no-cache",
        "subway",
        "timetable",
        "藤が丘",
        "--day",
        "weekday",
        "--line",
        "存在しない路線",
    ]);
    assert_eq!(out.status.code(), Some(3));
    let out = server.run(&[
        "--no-cache",
        "--quiet",
        "subway",
        "timetable",
        "藤が丘",
        "--day",
        "weekday",
        "--after",
        "27:59",
    ]);
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("予定時刻"));
    assert!(String::from_utf8_lossy(&out.stdout).contains("ありません"));
}

#[test]
fn next_reports_partial_failure_without_discarding_initial_departure() {
    let server = Server::new(|request| {
        if request.contains("station_master") {
            Response::json(r#"[{"id":"50","name":"藤が丘"}]"#)
        } else if request.contains("setting") {
            let mut r = Response::json("{}");
            r.status = 400;
            r
        } else {
            Response::json(r#"{"東山線":[{"RAILWAY":"高畑方面","DIAGRAM":{"平日":{"23":[59]}}}]}"#)
        }
    });
    let out = server.run(&[
        "--no-cache",
        "--json",
        "--quiet",
        "subway",
        "next",
        "藤が丘",
        "--at",
        "23:59",
        "--day",
        "weekday",
    ]);
    assert_eq!(out.status.code(), Some(4));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["complete"], false);
    assert_eq!(v["data"]["departures"][0]["time"], "23:59");
    assert_eq!(v["errors"][0]["exit_code"], 4);
    assert!(!out.stderr.is_empty());
}

#[test]
fn refresh_revalidates_etag_and_does_not_fallback_to_stale_cache() {
    let mode = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let state = mode.clone();
    let server = Server::new(move |request| {
        let mut r = Response::json(if request.contains("station_name") {
            r#"{"上社":"11180"}"#
        } else {
            r#"{"POLES":null}"#
        })
        .header("ETag: \"v1\"\r\nCache-Control: max-age=3600\r\n");
        match state.load(std::sync::atomic::Ordering::Relaxed) {
            1 => {
                r.status = 304;
                r.body.clear();
            }
            2 => r.status = 503,
            _ => (),
        }
        r
    });
    let cache = std::env::temp_dir().join(format!("nkotsu-refresh-cache-{}", std::process::id()));
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_nkotsu"))
            .env("NKOTSU_BASE_URL", &server.url)
            .env("NKOTSU_CACHE_DIR", &cache)
            .args(args)
            .output()
            .unwrap()
    };
    assert!(run(&["bus", "stop", "上社"]).status.success());
    mode.store(1, std::sync::atomic::Ordering::Relaxed);
    assert!(run(&["--refresh", "bus", "stop", "上社"]).status.success());
    assert_eq!(server.requests.lock().unwrap().len(), 4);
    assert!(
        server.requests.lock().unwrap()[2]
            .to_lowercase()
            .contains("if-none-match: \"v1\"")
    );
    mode.store(2, std::sync::atomic::Ordering::Relaxed);
    assert_eq!(
        run(&["--refresh", "bus", "stop", "上社"]).status.code(),
        Some(4)
    );
    assert_eq!(
        run(&["--no-cache", "bus", "stop", "上社"]).status.code(),
        Some(4)
    );
    for file in std::fs::read_dir(&cache).unwrap() {
        std::fs::remove_file(file.unwrap().path()).unwrap();
    }
    std::fs::remove_dir(&cache).unwrap();
}

#[test]
fn route_checks_upstream_status_and_malformed_xml() {
    for (xml, code) in [
        (
            "<NorikaeData><Nstatus><Status>false</Status><Err><ErrorCode>NO_ROUTE</ErrorCode></Err></Nstatus></NorikaeData>",
            6,
        ),
        ("<NorikaeData><Nstatus><Status>true</Status></Nstatus>", 5),
    ] {
        let server = Server::new(move |request| {
            if request.contains("StationInfos") {
                Response::json(
                    r#"[{"station_cd":"001","station_div":"2","station_name":"藤が丘","latitude":35.0,"longitude":137.0}]"#,
                )
            } else if request.contains("SETTING") {
                Response {
                    status: 200,
                    mime: "text/javascript",
                    headers: String::new(),
                    body: r#"var s={guid:"public",apipath:"/NagoyaRoute/PRD/"};"#.into(),
                }
            } else {
                Response {
                    status: 200,
                    mime: "text/xml",
                    headers: String::new(),
                    body: xml.into(),
                }
            }
        });
        let out = server.run(&["--no-cache", "route", "藤が丘", "藤が丘", "--subway"]);
        assert_eq!(
            out.status.code(),
            Some(code),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

#[test]
fn live_valid_route_and_pole_with_no_service_is_successful_empty_result() {
    let server = Server::new(|request| {
        Response::json(if request.contains("station_name") {
            r#"{"上社":"11180"}"#
        } else if request.contains("/stations/") {
            r#"{"POLES":{"002":{"name":"4番"},"003":{"name":"3番"}}}"#
        } else if request.contains("bus_service_status") {
            r#"[{"bus_service_active":true}]"#
        } else if request.contains("/busstops/") {
            r#"{"POLES":[{"NORIBA":"4番","KEITOS":["001"]},{"NORIBA":"3番","KEITOS":["002"]}]}"#
        } else if request.contains("/keitos/001") {
            r#"{"NAME":"上社１２","BUSSTOPS":[]}"#
        } else {
            r#"{"NAME":"上社１１","BUSSTOPS":[]}"#
        })
    });
    let out = server.run(&[
        "--no-cache",
        "--verbose",
        "--json",
        "bus",
        "live",
        "上社",
        "--route",
        "上社12",
        "--pole",
        "3番",
    ]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["data"]["routes"].as_array().unwrap().len(), 0);
    assert!(
        !server
            .requests
            .lock()
            .unwrap()
            .iter()
            .any(|r| r.contains("realtime_json"))
    );
}

#[test]
fn live_failed_service_settings_are_unknown_instead_of_stopped() {
    let server = Server::new(|request| {
        if request.contains("bus_service_status") {
            let mut r = Response::json("{}");
            r.status = 400;
            r
        } else {
            Response::json(if request.contains("station_name") {
                r#"{"上社":"11180"}"#
            } else if request.contains("/stations/") {
                r#"{"POLES":{}}"#
            } else {
                r#"{"POLES":[]}"#
            })
        }
    });
    let out = server.run(&["--no-cache", "--json", "bus", "live", "上社"]);
    assert_eq!(out.status.code(), Some(4));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(v["data"]["service_active"].is_null());
    let out = server.run(&["--no-cache", "bus", "live", "上社"]);
    assert!(!String::from_utf8_lossy(&out.stdout).contains("サービス停止中"));
}

#[test]
fn status_valid_line_with_no_articles_is_successful_and_raw_survives_parse_errors() {
    let server = Server::new(|_| Response::json("[]"));
    assert!(
        server
            .run(&["--no-cache", "status", "--line", "東山線"])
            .status
            .success()
    );
    let server = Server::new(|_| Response::json("{invalid JSON\n"));
    let out = server.run(&["--no-cache", "--raw", "status"]);
    assert_eq!(out.status.code(), Some(5));
    let raw: std::collections::BTreeMap<String, String> =
        serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(raw.values().next().unwrap(), "{invalid JSON\n");
}
