use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command;

fn place_response(request: &str) -> Response {
    Response::json(if request.contains("station_name.json") {
        r#"{"藤が丘":53030,"上社":11180,"金屋":42}"#
    } else if request.contains("station_master.json") {
        r#"[{"id":"050","name":"藤が丘","codes":["H22"]},{"id":"001","name":"Ｆ駅","codes":["H08","S02"]},{"id":"002","name":"F駅前","codes":["H09"]}]"#
    } else if request.contains("station_latlng.json") {
        "\u{feff}[\n {\"name\":\"藤が丘(名古屋市地下鉄)\",\"lat\":35.182355,\"lng\":137.021418},\n {\"name\":\"藤が丘(名古屋市バス)\",\"lat\":35.18,\"lng\":137.02},\n {\"name\":\"上社\",\"lat\":35.18,\"lng\":137.0},\n {\"name\":\"Ｆ駅(名古屋市地下鉄)\",\"lat\":35.19,\"lng\":137.0},\n {\"name\":\"金屋(名古屋市バス)\",\"lat\":35.18,\"lng\":137.01},\n {\"name\":\"金屋(ゆとりーとライン)\",\"lat\":35.18,\"lng\":137.01}\n]\n"
    } else {
        panic!("unexpected request: {request}")
    })
}

#[test]
fn location_resolves_facilities_without_exposing_internal_ids() {
    let server = Server::new(place_response);
    let out = server.run(&["location", "藤が丘(名古屋市地下鉄)", "--json", "--no-cache"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        value,
        serde_json::json!({
            "schema_version":1,"complete":true,"errors":[],
            "data":{"name":"藤が丘","qualified_name":"藤が丘(名古屋市地下鉄)","type":"subway","latitude":35.182355,"longitude":137.021418}
        })
    );
    let out = server.run(&["location", "藤が丘", "--json", "--no-cache"]);
    assert_eq!(out.status.code(), Some(3));
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(value["errors"][0]["code"], "ambiguous");
    let out = server.run(&["location", "上社", "--type", "bus", "--no-cache"]);
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("上社 [市バス]\n緯度: 35.18\n経度: 137"));
}

#[test]
fn location_keeps_name_identity_when_coordinates_are_invalid_or_missing() {
    let server = Server::new(|request| {
        if request.contains("station_latlng") {
            Response::json(
                r#"[{"name":"藤が丘(名古屋市地下鉄)","lat":91,"lng":137},{"name":"藤が丘(名古屋市バス)","lat":35,"lng":137},{"name":"上社(名古屋市バス)","lat":35,"lng":137}]"#,
            )
        } else {
            place_response(request)
        }
    });
    for (name, kind, code, failure) in [
        ("藤が丘", None, 3, "ambiguous"),
        ("藤が丘", Some("subway"), 5, "invalid_response"),
        ("Ｆ駅", Some("subway"), 5, "invalid_response"),
        ("存在しない", None, 3, "not_found"),
    ] {
        let mut args = vec!["location", name, "--json", "--no-cache"];
        if let Some(kind) = kind {
            args.extend(["--type", kind]);
        }
        let out = server.run(&args);
        assert_eq!(out.status.code(), Some(code));
        let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(value["errors"][0]["code"], failure);
        assert!(value["data"].is_null());
    }
    assert!(
        server
            .run(&["location", "上社", "--type", "bus", "--no-cache"])
            .status
            .success()
    );
}

#[test]
fn location_validates_types_before_fetching_and_raw_keeps_all_master_bodies() {
    let server = Server::new(place_response);
    let out = server.run(&[
        "location",
        "藤が丘(名古屋市地下鉄)",
        "--type",
        "bus",
        "--json",
        "--no-cache",
    ]);
    assert_eq!(out.status.code(), Some(2));
    assert!(server.requests.lock().unwrap().is_empty());
    let out = server.run(&["location", "駅 (名古屋市地下鉄)", "--json", "--no-cache"]);
    assert_eq!(out.status.code(), Some(3));
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(
        value["errors"][0]["message"]
            .as_str()
            .unwrap()
            .contains("Ｆ駅(名古屋市地下鉄)")
    );
    let out = server.run(&["location", "F駅(名古屋市地下鉄)", "--json", "--no-cache"]);
    assert!(out.status.success());
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(value["data"]["name"], "Ｆ駅");
    let out = server.run(&["location", "藤が丘(名古屋市地下鉄)", "--raw", "--no-cache"]);
    assert!(out.status.success());
    let raw: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(raw.as_object().unwrap().len(), 3);
    assert_eq!(
        raw[format!(
            "{}/STATION_DATA/station_infos/station_latlng.json",
            server.url
        )],
        place_response("station_latlng.json").body
    );
    assert_eq!(
        server.run(&["location", "F駅", "--no-cache"]).status.code(),
        Some(0)
    );
}

#[test]
fn location_requires_all_masters_and_rejects_conflicting_coordinates() {
    for path in ["station_latlng", "station_master", "station_name"] {
        let server = Server::new(move |request| {
            if request.contains(path) {
                let mut r = Response::json("{}");
                r.status = 400;
                r
            } else {
                place_response(request)
            }
        });
        let out = server.run(&["location", "藤が丘(名古屋市地下鉄)", "--json", "--no-cache"]);
        assert_eq!(out.status.code(), Some(4));
        let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert!(value["data"].is_null());
    }
    let server = Server::new(|request| {
        if request.contains("station_latlng") {
            Response::json(
                r#"[{"name":"藤が丘(名古屋市地下鉄)","lat":35,"lng":137},{"name":"藤が丘(名古屋市地下鉄)","lat":36,"lng":137}]"#,
            )
        } else {
            place_response(request)
        }
    });
    assert_eq!(
        server
            .run(&["location", "藤が丘(名古屋市地下鉄)", "--no-cache"])
            .status
            .code(),
        Some(5)
    );
}

#[test]
fn nearby_returns_rounded_straight_line_distances_and_preserves_other_facility_types() {
    let server = Server::new(place_response);
    let out = server.run(&[
        "nearby",
        "藤が丘",
        "--origin-type",
        "subway",
        "--type",
        "bus",
        "--limit",
        "1",
        "--json",
        "--no-cache",
    ]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(value["data"]["origin"]["type"], "subway");
    assert_eq!(
        value["data"]["places"],
        serde_json::json!([{
            "name":"藤が丘","qualified_name":"藤が丘(名古屋市バス)","type":"bus","latitude":35.18,"longitude":137.02,"distance_m":292
        }])
    );
}

#[test]
fn nearby_filters_unrounded_radius_before_limit_and_sorts_public_distance_ties() {
    let server = Server::new(|request| {
        Response::json(if request.contains("station_name") {
            r#"{"基準":1,"Ａ近":2,"Ｂ近":3,"半径端":4,"遠":5}"#
        } else if request.contains("station_master") {
            r#"[{"name":"基準","codes":["H01"]}]"#
        } else {
            r#"[{"name":"基準(名古屋市地下鉄)","lat":0,"lng":0},{"name":"基準(名古屋市バス)","lat":0,"lng":0},{"name":"Ａ近","lat":0,"lng":0.000009},{"name":"Ｂ近","lat":0,"lng":0.0000085},{"name":"半径端","lat":0.0045,"lng":0},{"name":"遠","lat":0,"lng":1}]"#
        })
    });
    let out = server.run(&["nearby", "基準(名古屋市地下鉄)", "--json", "--no-cache"]);
    assert!(out.status.success());
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let places = value["data"]["places"].as_array().unwrap();
    assert_eq!(
        places
            .iter()
            .map(|p| p["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["基準", "Ａ近", "Ｂ近", "半径端", "遠"]
    );
    assert_eq!(
        places
            .iter()
            .map(|p| p["distance_m"].as_u64().unwrap())
            .collect::<Vec<_>>(),
        [0, 1, 1, 500, 111195]
    );
    let out = server.run(&[
        "nearby",
        "基準(名古屋市地下鉄)",
        "--radius",
        "500",
        "--limit",
        "10",
        "--json",
        "--no-cache",
    ]);
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(value["data"]["places"].as_array().unwrap().len(), 3);
    let out = server.run(&[
        "nearby",
        "基準(名古屋市地下鉄)",
        "--radius",
        "1",
        "--limit",
        "1",
        "--json",
        "--no-cache",
    ]);
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(value["data"]["places"].as_array().unwrap().len(), 1);
    assert_eq!(value["data"]["places"][0]["name"], "基準");
    let out = server.run(&[
        "nearby",
        "基準(名古屋市地下鉄)",
        "--type",
        "subway",
        "--no-cache",
    ]);
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("該当する周辺の交通施設はありません"));
    assert_eq!(
        server.run(&["nearby", "基準", "--no-cache"]).status.code(),
        Some(3)
    );
    let out = server.run(&["nearby", "基準(名古屋市地下鉄)", "--raw", "--no-cache"]);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&out.stdout)
            .unwrap()
            .as_object()
            .unwrap()
            .len(),
        3
    );
}

#[test]
fn nearby_reports_invalid_candidates_even_outside_radius_or_limit() {
    let server = Server::new(|request| {
        Response::json(if request.contains("station_name") {
            r#"{"正常":1,"不正":2}"#
        } else if request.contains("station_master") {
            r#"[{"name":"基準"},{"name":"地下鉄不正"}]"#
        } else {
            r#"[{"name":"基準(名古屋市地下鉄)","lat":35,"lng":137},{"name":"正常","lat":35,"lng":137},{"name":"不正","lat":"NaN","lng":137},{"name":"地下鉄不正(名古屋市地下鉄)","lat":35,"lng":181}]"#
        })
    });
    let out = server.run(&[
        "nearby",
        "基準",
        "--type",
        "bus",
        "--radius",
        "1",
        "--limit",
        "1",
        "--json",
        "--no-cache",
    ]);
    assert_eq!(out.status.code(), Some(5));
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(value["complete"], false);
    assert_eq!(value["errors"].as_array().unwrap().len(), 1);
    assert_eq!(value["data"]["places"][0]["name"], "正常");
    assert_eq!(
        value["data"]["origin"]["qualified_name"],
        "基準(名古屋市地下鉄)"
    );
}

#[test]
fn search_aggregates_suggestions_without_publishing_bus_ids_or_platforms() {
    let server = Server::new(|request| {
        assert!(request.contains("marge=pole") && request.contains("next="));
        Response::json(if request.contains("div=1") {
            r#"[{"station_cd":"internal-bus","station_div":"1","station_num":"53030","station_name":"藤が丘","line_name":"藤丘12,藤丘11","latitude":35.18,"longitude":137.02,"pole_name":"誤ったのりば"}]"#
        } else {
            r#"[{"station_cd":"internal-subway","station_div":"2","station_num":"H22|S02","station_name":"藤が丘","line_name":"東山線,桜通線","latitude":35.182355,"longitude":137.021418},{"station_cd":"internal-subway","station_div":"2","station_num":"H22","station_name":"藤が丘","line_name":"東山線","latitude":35.182355,"longitude":137.021418},{"station_cd":"later","station_div":"2","station_num":"H01","station_name":"藤が丘前"}]"#
        })
    });
    let out = server.run(&["search", " 藤が丘 ", "--limit", "2", "--json", "--no-cache"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(value["data"]["query"], "藤が丘");
    assert_eq!(value["data"]["total"], 3);
    assert_eq!(value["data"]["results"].as_array().unwrap().len(), 2);
    assert_eq!(value["data"]["results"][0]["type"], "bus");
    assert_eq!(value["data"]["results"][0]["codes"], serde_json::json!([]));
    assert_eq!(
        value["data"]["results"][0]["lines"],
        serde_json::json!(["藤丘11", "藤丘12"])
    );
    assert_eq!(
        value["data"]["results"][1]["codes"],
        serde_json::json!(["H22", "S02"])
    );
    assert_eq!(
        value["data"]["results"][1]["qualified_name"],
        "藤が丘(名古屋市地下鉄)"
    );
    let text = String::from_utf8(out.stdout).unwrap();
    for private in ["internal-bus", "internal-subway", "53030", "pole_name"] {
        assert!(!text.contains(private));
    }
}

#[test]
fn station_keeps_codes_and_optional_coordinates_and_accepts_qualified_nfkc_names() {
    let server = Server::new(place_response);
    let out = server.run(&["station", " F駅(名古屋市地下鉄) ", "--json", "--no-cache"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(value["data"]["name"], "Ｆ駅");
    assert_eq!(value["data"]["codes"], serde_json::json!(["H08", "S02"]));
    assert!(value["data"].get("id").is_none());
    let out = server.run(&["station", "藤が丘", "--no-cache"]);
    assert!(out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stdout)
            .contains("駅記号: H22\n緯度: 35.182355\n経度: 137.021418")
    );
    let out = server.run(&["station", "F駅前", "--json", "--no-cache"]);
    assert!(out.status.success());
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(value["data"]["latitude"].is_null() && value["data"]["longitude"].is_null());
    let out = server.run(&["station", "藤が丘", "--raw", "--no-cache"]);
    let raw: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(raw.as_object().unwrap().len(), 2);
    assert_eq!(
        raw[format!(
            "{}/STATION_DATA/station_infos/station_latlng.json",
            server.url
        )],
        place_response("station_latlng.json").body
    );
    assert_eq!(
        server.run(&["station", "駅", "--no-cache"]).status.code(),
        Some(3)
    );
    assert_eq!(
        server.run(&["station", "未知", "--no-cache"]).status.code(),
        Some(3)
    );
}

#[test]
fn search_distinguishes_partial_fetches_total_failure_and_successful_empty_results() {
    for (bus_status, subway_status, data_present, errors) in [
        (400, 200, true, 1),
        (200, 400, true, 1),
        (400, 400, false, 2),
    ] {
        let server = Server::new(move |request| {
            let mut response = Response::json("[]");
            response.status = if request.contains("div=1") {
                bus_status
            } else {
                subway_status
            };
            response
        });
        let out = server.run(&["search", "藤が丘", "--json", "--no-cache"]);
        assert_eq!(out.status.code(), Some(4));
        let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(value["complete"], false);
        assert_eq!(!value["data"].is_null(), data_present);
        assert_eq!(value["errors"].as_array().unwrap().len(), errors);
        if data_present {
            assert_eq!(value["data"]["total"], 0);
        }
        let failing_kind = if bus_status == 400 { "bus" } else { "subway" };
        let out = server.run(&[
            "search",
            "藤が丘",
            "--type",
            failing_kind,
            "--json",
            "--no-cache",
        ]);
        let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert!(value["data"].is_null());
        assert_eq!(out.status.code(), Some(4));
    }
    let server = Server::new(|_| Response::json("[]"));
    let out = server.run(&["search", "未知", "--json", "--no-cache"]);
    assert!(out.status.success());
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(value["data"]["total"], 0);
    assert_eq!(value["complete"], true);
}

#[test]
fn search_preserves_names_when_coordinates_fail_and_drops_unidentifiable_candidates() {
    let server = Server::new(|_| {
        Response::json(
            r#"[
        {"station_cd":"good","station_div":"2","station_name":"正常","latitude":35,"longitude":137},
        {"station_cd":"missing","station_div":"2","station_name":"座標なし"},
        {"station_cd":"broken","station_div":"2","station_name":"不正座標","latitude":1e309,"longitude":137},
        {"station_cd":"half","station_div":"2","station_name":"片方座標","latitude":35},
        {"station_cd":"conflict","station_div":"2","station_name":"矛盾座標","latitude":35,"longitude":137},
        {"station_cd":"conflict","station_div":"2","station_name":"矛盾座標","latitude":36,"longitude":137},
        {"station_div":"2","station_name":"IDなし"},
        {"station_cd":"name-conflict","station_div":"2","station_name":"駅A"},
        {"station_cd":"name-conflict","station_div":"2","station_name":"駅B"}
    ]"#,
        )
    });
    let out = server.run(&["search", "駅", "--type", "subway", "--json", "--no-cache"]);
    assert_eq!(out.status.code(), Some(5));
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(value["complete"], false);
    assert_eq!(value["data"]["total"], 5);
    assert_eq!(value["errors"].as_array().unwrap().len(), 5);
    for place in value["data"]["results"].as_array().unwrap() {
        if place["name"] == "正常" {
            assert_eq!(place["latitude"], 35.0);
        } else {
            assert!(place["latitude"].is_null() && place["longitude"].is_null());
        }
    }
    let out = server.run(&[
        "search",
        "駅",
        "--type",
        "subway",
        "--limit",
        "1",
        "--json",
        "--no-cache",
    ]);
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(value["errors"].as_array().unwrap().len(), 5);
    assert_eq!(value["data"]["total"], 5);
    assert_eq!(value["data"]["results"].as_array().unwrap().len(), 1);
    let server = Server::new(|_| Response::json(r#"[{"station_name":"不正"}]"#));
    let out = server.run(&["search", "不正", "--type", "subway", "--json", "--no-cache"]);
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(value["data"]["total"], 0);
    assert_eq!(value["complete"], false);
    assert_eq!(out.status.code(), Some(5));
}

#[test]
fn search_normalizes_queries_filters_types_and_keeps_raw_bodies_without_persistent_cache() {
    let body = "\u{feff}[ {\"station_name\":\"Ｆ駅\",\"station_cd\":\"private\",\"station_div\":2,\"station_num\":\"H01\"} ]\n";
    let server = Server::new(move |request| {
        assert!(
            request.contains("key=F") && request.contains("div=2") && !request.contains("div=1")
        );
        Response::json(body).header("Cache-Control: max-age=3600\r\n")
    });
    let cache = std::env::temp_dir().join(format!("nkotsu-search-cache-{}", std::process::id()));
    for _ in 0..2 {
        let out = Command::new(env!("CARGO_BIN_EXE_nkotsu"))
            .env("NKOTSU_BASE_URL", &server.url)
            .env("NKOTSU_CACHE_DIR", &cache)
            .args(["search", " Ｆ ", "--type", "subway", "--raw"])
            .output()
            .unwrap();
        assert!(out.status.success());
        let raw: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(raw.as_object().unwrap().len(), 1);
        assert_eq!(raw.as_object().unwrap().values().next().unwrap(), body);
    }
    assert_eq!(server.requests.lock().unwrap().len(), 2);
    assert!(!cache.exists());
    let out = server.run(&["search", " Ｆ ", "--type", "subway", "--no-cache"]);
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("Ｆ駅 [地下鉄]") && text.contains("Ｆ駅(名古屋市地下鉄)"));
    assert!(!text.contains("緯度") && !text.contains("経度"));
}

#[test]
fn place_index_keeps_master_names_that_collide_after_normalization() {
    let server = Server::new(|request| {
        if request.contains("station_master") {
            Response::json(r#"[{"name":"Ｆ駅","codes":["H01"]},{"name":"F駅","codes":["H02"]}]"#)
        } else if request.contains("station_name") {
            Response::json("{}")
        } else {
            Response::json(r#"[{"name":"Ｆ駅(名古屋市地下鉄)","lat":35,"lng":137}]"#)
        }
    });
    for command in ["location", "nearby", "station"] {
        let out = server.run(&[command, "F駅(名古屋市地下鉄)", "--json", "--no-cache"]);
        assert_eq!(out.status.code(), Some(3), "{command}");
        let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(value["errors"][0]["code"], "ambiguous");
    }
}

#[test]
fn search_retains_identifiable_facilities_with_invalid_codes_or_lines() {
    let server = Server::new(|_| {
        Response::json(
            r#"[{"station_cd":"a","station_div":"2","station_name":"駅","station_num":123,"line_name":{},"latitude":35,"longitude":137}]"#,
        )
    });
    let out = server.run(&["search", "駅", "--type", "subway", "--json", "--no-cache"]);
    assert_eq!(out.status.code(), Some(5));
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(value["data"]["total"], 1);
    assert_eq!(value["data"]["results"][0]["name"], "駅");
    assert_eq!(value["data"]["results"][0]["latitude"], 35.0);
    assert_eq!(value["data"]["results"][0]["codes"], serde_json::json!([]));
    assert_eq!(value["data"]["results"][0]["lines"], serde_json::json!([]));
    assert_eq!(value["errors"].as_array().unwrap().len(), 2);
}

#[test]
fn place_arguments_fail_before_network_requests() {
    let server = Server::new(|_| panic!("invalid arguments must not fetch"));
    for args in [
        vec!["search", ""],
        vec!["search", "　 "],
        vec!["location", " (名古屋市地下鉄)"],
        vec!["station", " (名古屋市地下鉄)"],
        vec!["search", "駅", "--limit", "0"],
        vec!["search", "駅", "--type", "train"],
        vec!["nearby", "駅", "--radius", "0"],
        vec!["nearby", "駅", "--limit", "0"],
        vec!["nearby", "藤が丘(名古屋市地下鉄)", "--origin-type", "bus"],
    ] {
        let out = server.run(&[args.as_slice(), &["--json", "--no-cache"]].concat());
        assert_eq!(out.status.code(), Some(2));
        let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(value["errors"][0]["code"], "invalid_arguments");
    }
    assert!(server.requests.lock().unwrap().is_empty());
}

#[test]
fn station_retains_basic_information_on_coordinate_errors_but_requires_its_master() {
    for (body, status, exit_code, complete) in [
        ("[]", 200, 0, true),
        (
            r#"[{"name":"藤が丘(名古屋市地下鉄)","lat":35}]"#,
            200,
            5,
            false,
        ),
        (
            r#"[{"name":"藤が丘(名古屋市地下鉄)","lat":1e309,"lng":137}]"#,
            200,
            5,
            false,
        ),
        (
            r#"[{"name":"藤が丘(名古屋市地下鉄)","lat":35,"lng":137},{"name":"藤が丘(名古屋市地下鉄)","lat":36,"lng":137}]"#,
            200,
            5,
            false,
        ),
        ("{broken", 200, 5, false),
        ("{}", 400, 4, false),
    ] {
        let server = Server::new(move |request| {
            if request.contains("station_latlng") {
                let mut r = Response::json(body);
                r.status = status;
                r
            } else {
                place_response(request)
            }
        });
        let out = server.run(&["station", "藤が丘", "--json", "--no-cache"]);
        assert_eq!(out.status.code(), Some(exit_code));
        let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(value["complete"], complete);
        assert_eq!(value["data"]["codes"], serde_json::json!(["H22"]));
        assert!(value["data"]["latitude"].is_null() && value["data"]["longitude"].is_null());
    }
    let server = Server::new(|_| {
        let mut r = Response::json("{}");
        r.status = 400;
        r
    });
    let out = server.run(&["station", "藤が丘", "--json", "--no-cache"]);
    assert_eq!(out.status.code(), Some(4));
    assert!(serde_json::from_slice::<serde_json::Value>(&out.stdout).unwrap()["data"].is_null());
}

#[test]
fn place_commands_share_master_cache_and_refresh_conditionally() {
    let server = Server::new(|request| {
        if request.to_lowercase().contains("if-none-match:") {
            let mut response = Response::json("");
            response.status = 304;
            response
        } else {
            place_response(request).header("Cache-Control: max-age=3600\r\nETag: \"places-v1\"\r\n")
        }
    });
    let cache = std::env::temp_dir().join(format!("nkotsu-places-cache-{}", std::process::id()));
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_nkotsu"))
            .env("NKOTSU_BASE_URL", &server.url)
            .env("NKOTSU_CACHE_DIR", &cache)
            .args(args)
            .output()
            .unwrap()
    };
    for args in [
        vec!["station", "藤が丘"],
        vec!["location", "藤が丘(名古屋市地下鉄)"],
        vec!["nearby", "藤が丘(名古屋市地下鉄)", "--type", "bus"],
    ] {
        assert!(run(&args).status.success());
    }
    assert_eq!(server.requests.lock().unwrap().len(), 3);
    assert!(
        run(&["location", "藤が丘(名古屋市地下鉄)", "--refresh"])
            .status
            .success()
    );
    assert_eq!(server.requests.lock().unwrap().len(), 6);
    assert!(run(&["station", "藤が丘", "--no-cache"]).status.success());
    assert_eq!(server.requests.lock().unwrap().len(), 8);
    for file in std::fs::read_dir(&cache).unwrap() {
        std::fs::remove_file(file.unwrap().path()).unwrap();
    }
    std::fs::remove_dir(cache).unwrap();
}

#[test]
fn place_index_does_not_guess_unqualified_shared_names_or_merge_other_transport_facilities() {
    let server = Server::new(|request| {
        if request.contains("station_latlng") {
            Response::json(
                r#"[{"name":"藤が丘","lat":35,"lng":137},{"name":"上社","lat":35,"lng":137},{"name":"上社","lat":35,"lng":137},{"name":"金屋(名古屋市バス)","lat":35,"lng":137},{"name":"金屋(ゆとりーとライン)","lat":36,"lng":138}]"#,
            )
        } else {
            place_response(request)
        }
    });
    assert_eq!(
        server
            .run(&["location", "藤が丘", "--no-cache"])
            .status
            .code(),
        Some(3)
    );
    assert_eq!(
        server
            .run(&["location", "藤が丘", "--type", "subway", "--no-cache"])
            .status
            .code(),
        Some(5)
    );
    assert!(
        server
            .run(&["location", "上社", "--type", "bus", "--no-cache"])
            .status
            .success()
    );
    let out = server.run(&["location", "金屋", "--json", "--no-cache"]);
    assert!(out.status.success());
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(value["data"]["qualified_name"], "金屋(名古屋市バス)");
    assert_eq!(value["data"]["latitude"], 35.0);
}

fn teiki_response(request: &str) -> Response {
    Response::json(if request.contains("/teiki/station.json") {
        r#"{"021":"藤が丘","007":"名古屋","008":"Ｆ駅"}"#
    } else if request.contains("/teiki/ends/021.json") {
        r#"{"007":184467440737095516160}"#
    } else if request.contains("/teiki/ends/008.json") {
        r#"{"007":"184467440737095516160"}"#
    } else if request.contains("/teiki/route/184467440737095516160.json") {
        r#"{"A迂回":{"FARE":340,"TEIKI":81,"DISTANCE":199},"B直通":{"FARE":310,"TEIKI":80},"C迂回":{"FARE":340,"TEIKI":81}}"#
    } else if request.contains("/teiki/teiki/81.json") {
        r#"{"大学生":{"1":6440,"3":18360,"6":34780},"通勤":{"1":12060,"3":34380,"6":65130}}"#
    } else if request.contains("/teiki/teiki/80.json") {
        r#"{"大学生":{"1":6200,"3":17670,"6":33480}}"#
    } else if request.contains("/teiki/use_shi_bus.json") {
        r#"{"81":21}"#
    } else if request.contains("/teiki/teiki/21.json") {
        r#"{"大学生":{"1":9480,"3":27000,"6":51160}}"#
    } else {
        panic!("unexpected request: {request}")
    })
}

#[test]
fn fare_returns_all_named_routes_from_its_own_station_master() {
    let server = Server::new(teiki_response);
    let out = server.run(&["fare", " 藤が丘 ", "名古屋", "--json", "--no-cache"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        v,
        serde_json::json!({
            "schema_version":1,"complete":true,"errors":[],
            "data":{"from":"藤が丘","to":"名古屋","routes":[
                {"name":"A迂回","fare_yen":340},
                {"name":"B直通","fare_yen":310},
                {"name":"C迂回","fare_yen":340}
            ]}
        })
    );
    assert_eq!(server.requests.lock().unwrap().len(), 3);
}

#[test]
fn pass_keeps_all_routes_and_filters_dynamic_types_and_months() {
    let server = Server::new(teiki_response);
    let out = server.run(&[
        "pass",
        "藤が丘",
        "名古屋",
        "--type",
        "大学",
        "--months",
        "1",
        "--json",
        "--no-cache",
    ]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["complete"], true);
    assert_eq!(
        v["data"]["routes"],
        serde_json::json!([
            {"name":"A迂回","types":[{"name":"大学生","prices":[{"months":1,"yen":6440}]}]},
            {"name":"B直通","types":[{"name":"大学生","prices":[{"months":1,"yen":6200}]}]},
            {"name":"C迂回","types":[{"name":"大学生","prices":[{"months":1,"yen":6440}]}]}
        ])
    );
    let requests = server.requests.lock().unwrap();
    assert_eq!(
        requests
            .iter()
            .filter(|r| r.contains("/teiki/teiki/81.json"))
            .count(),
        1
    );
    assert_eq!(requests.len(), 5);
}

#[test]
fn delay_cert_sorts_target_datetimes_and_keeps_delay_text_and_urls() {
    let server = Server::new(|request| {
        assert!(request.contains("/datas/traffic_delay_certificate.json"));
        Response::json(
            r#"[
            {"delay_id":"old","rosen_name":"東山線","sort_no":1,"delay_datetime":"2026-09-08T00:00:00","title":"始発から","max_delay_time":"60分以上"},
            {"delay_id":"later","rosen_name":"名城線・名港線","sort_no":0,"delay_datetime":"2026-09-15T00:00:00","title":"午後","max_delay_time":"10分"}
        ]"#,
        )
    });
    let out = server.run(&["delay-cert", "--json", "--no-cache"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["schema_version"], 1);
    assert_eq!(v["complete"], true);
    assert!(
        v["data"]["fetched_at"]
            .as_str()
            .unwrap()
            .ends_with("+09:00")
    );
    assert_eq!(
        v["data"]["certificates"],
        serde_json::json!([
            {"line":"名城線・名港線","date":"2026-09-15","title":"午後","max_delay_time":"10分","url":"https://www.kotsu.city.nagoya.jp/rp/subway/delay_certificate.html?delay_id=later"},
            {"line":"東山線","date":"2026-09-08","title":"始発から","max_delay_time":"60分以上","url":"https://www.kotsu.city.nagoya.jp/rp/subway/delay_certificate.html?delay_id=old"}
        ])
    );
}

#[test]
fn fare_resolves_nfkc_names_and_route_filters_without_choosing_ambiguous_routes() {
    let server = Server::new(teiki_response);
    let out = server.run(&[
        "fare",
        " F駅 ",
        "名古屋",
        "--route",
        "Ｂ直通",
        "--json",
        "--no-cache",
    ]);
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["data"]["from"], "Ｆ駅");
    assert_eq!(
        v["data"]["routes"],
        serde_json::json!([{"name":"B直通","fare_yen":310}])
    );
    for (from, to, route, code) in [
        ("藤が丘", "名古屋", "迂回", "ambiguous"),
        ("藤が丘", "名古屋", "存在しない", "not_found"),
        ("存在しない", "名古屋", "B直通", "not_found"),
        ("藤が丘", "存在しない", "B直通", "not_found"),
    ] {
        let out = server.run(&["fare", from, to, "--route", route, "--json", "--no-cache"]);
        assert_eq!(out.status.code(), Some(3));
        let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert!(v["data"].is_null());
        assert_eq!(v["errors"][0]["code"], code);
    }
}

#[test]
fn fare_keeps_unknown_prices_as_partial_results_and_preserves_raw_bodies() {
    let body = "\u{feff}{\n\"有効\":{\"FARE\":310},\"欠落\":{},\"不正\":{\"FARE\":-1}}\n";
    let server = Server::new(move |request| {
        if request.contains("/teiki/route/") {
            Response::json(body)
        } else {
            teiki_response(request)
        }
    });
    let out = server.run(&[
        "fare",
        "藤が丘",
        "名古屋",
        "--json",
        "--quiet",
        "--no-cache",
    ]);
    assert_eq!(out.status.code(), Some(5));
    assert!(!out.stderr.is_empty());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["complete"], false);
    assert_eq!(v["data"]["routes"].as_array().unwrap().len(), 3);
    assert!(v["data"]["routes"][0]["fare_yen"].is_null());
    assert_eq!(v["data"]["routes"][1]["fare_yen"], 310);
    assert!(v["data"]["routes"][2]["fare_yen"].is_null());
    assert_eq!(v["errors"].as_array().unwrap().len(), 2);
    let out = server.run(&["fare", "藤が丘", "名古屋", "--raw", "--no-cache"]);
    assert_eq!(out.status.code(), Some(5));
    let raw: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(raw.as_object().unwrap().len(), 3);
    assert_eq!(
        raw[format!(
            "{}/STATION_DATA/teiki/route/184467440737095516160.json",
            server.url
        )],
        body
    );
}

#[test]
fn fare_and_pass_valid_stations_without_a_connection_are_successful_empty_results() {
    let server = Server::new(teiki_response);
    for command in ["fare", "pass"] {
        let out = server.run(&[command, "藤が丘", "藤が丘", "--json", "--no-cache"]);
        assert!(out.status.success());
        let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(v["complete"], true);
        assert_eq!(v["data"]["routes"], serde_json::json!([]));
    }
    assert_eq!(server.requests.lock().unwrap().len(), 4);
    let server = Server::new(|request| {
        if request.contains("/teiki/route/") {
            Response::json("{}")
        } else {
            teiki_response(request)
        }
    });
    let out = server.run(&["fare", "藤が丘", "名古屋", "--no-cache"]);
    assert!(out.status.success());
    assert!(
        String::from_utf8(out.stdout)
            .unwrap()
            .contains("該当する運賃経路はありません")
    );
}

#[test]
fn fare_reuses_and_revalidates_static_cache_and_no_cache_disables_it() {
    let revalidate = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let state = revalidate.clone();
    let server = Server::new(move |request| {
        let mut response = teiki_response(request).header("ETag: \"fare-v1\"\r\n");
        if state.load(std::sync::atomic::Ordering::Relaxed) {
            assert!(
                request
                    .to_lowercase()
                    .contains("if-none-match: \"fare-v1\"")
            );
            response.status = 304;
            response.body.clear();
        }
        response
    });
    let cache = std::env::temp_dir().join(format!("nkotsu-fare-cache-{}", std::process::id()));
    let run = |flags: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_nkotsu"))
            .env("NKOTSU_BASE_URL", &server.url)
            .env("NKOTSU_CACHE_DIR", &cache)
            .args(["fare", "藤が丘", "名古屋", "--json"])
            .args(flags)
            .output()
            .unwrap()
    };
    assert!(run(&[]).status.success());
    assert!(run(&[]).status.success());
    assert_eq!(server.requests.lock().unwrap().len(), 3);
    revalidate.store(true, std::sync::atomic::Ordering::Relaxed);
    assert!(run(&["--refresh"]).status.success());
    assert_eq!(server.requests.lock().unwrap().len(), 6);
    revalidate.store(false, std::sync::atomic::Ordering::Relaxed);
    assert!(run(&["--no-cache"]).status.success());
    assert_eq!(server.requests.lock().unwrap().len(), 9);
    for entry in std::fs::read_dir(&cache).unwrap() {
        std::fs::remove_file(entry.unwrap().path()).unwrap();
    }
    std::fs::remove_dir(cache).unwrap();
}

#[test]
fn pass_lists_types_and_prices_and_resolves_type_once_across_routes() {
    let server = Server::new(teiki_response);
    let out = server.run(&["pass", "藤が丘", "名古屋", "--json", "--no-cache"]);
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["data"]["routes"][0]["types"].as_array().unwrap().len(), 2);
    assert_eq!(
        v["data"]["routes"][0]["types"][0]["prices"],
        serde_json::json!([
            {"months":1,"yen":6440},{"months":3,"yen":18360},{"months":6,"yen":34780}
        ])
    );
    for (months, yen) in [("1", 6440), ("3", 18360), ("6", 34780)] {
        let out = server.run(&[
            "pass",
            "藤が丘",
            "名古屋",
            "--route",
            "Ａ迂回",
            "--type",
            "大学生",
            "--months",
            months,
            "--json",
            "--no-cache",
        ]);
        assert!(out.status.success());
        let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(v["data"]["routes"].as_array().unwrap().len(), 1);
        assert_eq!(v["data"]["routes"][0]["types"][0]["prices"][0]["yen"], yen);
        assert_eq!(
            v["data"]["routes"][0]["types"][0]["prices"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }
    let server = Server::new(|request| {
        if request.contains("/teiki/teiki/80.json") {
            Response::json(r#"{"大学院生":{"1":6000,"3":16000,"6":30000}}"#)
        } else {
            teiki_response(request)
        }
    });
    for (input, code) in [("大学", "ambiguous"), ("未登録", "not_found")] {
        let out = server.run(&[
            "pass",
            "藤が丘",
            "名古屋",
            "--type",
            input,
            "--json",
            "--no-cache",
        ]);
        assert_eq!(out.status.code(), Some(3));
        let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(v["errors"][0]["code"], code);
        assert!(v["data"].is_null());
    }
}

#[test]
fn pass_partial_fetch_keeps_routes_and_requires_exact_type_when_candidates_are_missing() {
    let server = Server::new(|request| {
        let mut response = teiki_response(request);
        if request.contains("/teiki/teiki/81.json") {
            response.status = 400;
        }
        response
    });
    for input in [None, Some("大学生"), Some("大学"), Some("未登録")] {
        let mut args = vec![
            "pass",
            "藤が丘",
            "名古屋",
            "--json",
            "--quiet",
            "--no-cache",
        ];
        if let Some(input) = input {
            args.extend(["--type", input]);
        }
        let out = server.run(&args);
        assert_eq!(out.status.code(), Some(4));
        assert!(!out.stderr.is_empty());
        let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(v["complete"], false);
        assert_eq!(v["data"]["routes"].as_array().unwrap().len(), 3);
        assert_eq!(v["data"]["routes"][0]["types"], serde_json::json!([]));
        assert_eq!(v["data"]["routes"][2]["types"], serde_json::json!([]));
        if input.is_none() || input == Some("大学生") {
            assert_eq!(v["data"]["routes"][1]["types"][0]["prices"][0]["yen"], 6200);
        } else {
            assert_eq!(v["data"]["routes"][1]["types"], serde_json::json!([]));
        }
        assert_eq!(v["errors"].as_array().unwrap().len(), 2);
        assert!(
            v["errors"]
                .as_array()
                .unwrap()
                .iter()
                .all(|e| e["code"] == "http_error")
        );
    }
    assert_eq!(
        server
            .requests
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.contains("/teiki/teiki/81.json"))
            .count(),
        4
    );
    let server = Server::new(|request| {
        let mut response = teiki_response(request);
        if request.contains("/teiki/teiki/") {
            response.status = 400;
        }
        response
    });
    let out = server.run(&["pass", "藤が丘", "名古屋", "--json", "--no-cache"]);
    assert_eq!(out.status.code(), Some(4));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["data"]["routes"].as_array().unwrap().len(), 3);
    assert!(
        v["data"]["routes"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["types"] == serde_json::json!([]))
    );
}

#[test]
fn pass_with_bus_uses_mapped_class_and_does_not_fallback_for_missing_mapping() {
    let server = Server::new(teiki_response);
    let out = server.run(&[
        "pass",
        "藤が丘",
        "名古屋",
        "--with-bus",
        "--json",
        "--no-cache",
    ]);
    assert_eq!(out.status.code(), Some(3));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["complete"], false);
    assert_eq!(v["data"]["routes"][0]["types"][0]["prices"][0]["yen"], 9480);
    assert_eq!(v["data"]["routes"][1]["types"], serde_json::json!([]));
    assert_eq!(v["data"]["routes"][2]["types"][0]["prices"][0]["yen"], 9480);
    assert_eq!(v["errors"][0]["code"], "not_found");
    let requests = server.requests.lock().unwrap();
    assert_eq!(requests.len(), 5);
    assert!(
        !requests
            .iter()
            .any(|r| r.contains("/teiki/teiki/80.json") || r.contains("/teiki/teiki/81.json"))
    );
    drop(requests);
    let out = server.run(&[
        "pass",
        "藤が丘",
        "名古屋",
        "--with-bus",
        "--raw",
        "--no-cache",
    ]);
    let raw: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(raw.as_object().unwrap().len(), 5);
    assert!(raw[format!("{}/STATION_DATA/teiki/use_shi_bus.json", server.url)].is_string());
    let server = Server::new(|request| {
        if request.contains("use_shi_bus") {
            Response::json("{}")
        } else {
            teiki_response(request)
        }
    });
    let out = server.run(&[
        "pass",
        "藤が丘",
        "名古屋",
        "--with-bus",
        "--json",
        "--no-cache",
    ]);
    assert_eq!(out.status.code(), Some(3));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(
        v["data"]["routes"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["types"] == serde_json::json!([]))
    );
    assert_eq!(server.requests.lock().unwrap().len(), 4);
}

#[test]
fn pass_keeps_valid_periods_when_another_price_is_invalid_and_supports_new_type_names() {
    let server = Server::new(|request| {
        if request.contains("/teiki/teiki/81.json") {
            Response::json(r#"{"研究生":{"1":6440,"3":18360,"6":"不正"},"不明券種":null}"#)
        } else {
            teiki_response(request)
        }
    });
    let out = server.run(&[
        "pass",
        "藤が丘",
        "名古屋",
        "--route",
        "A迂回",
        "--type",
        "研究",
        "--json",
        "--no-cache",
    ]);
    assert_eq!(out.status.code(), Some(5));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        v["data"]["routes"][0]["types"],
        serde_json::json!([
            {"name":"研究生","prices":[{"months":1,"yen":6440},{"months":3,"yen":18360}]}
        ])
    );
    assert_eq!(v["errors"].as_array().unwrap().len(), 2);
    let out = server.run(&[
        "pass",
        "藤が丘",
        "名古屋",
        "--route",
        "A迂回",
        "--json",
        "--no-cache",
    ]);
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        v["data"]["routes"][0]["types"][0],
        serde_json::json!({"name":"不明券種","prices":[]})
    );
}

#[test]
fn pass_human_warns_about_eligibility_but_quiet_keeps_prices_and_json_has_no_qualification() {
    let server = Server::new(teiki_response);
    for quiet in [false, true] {
        let mut args = vec!["pass", "藤が丘", "名古屋", "--route", "A迂回", "--no-cache"];
        if quiet {
            args.push("--quiet");
        }
        let out = server.run(&args);
        assert!(out.status.success());
        let text = String::from_utf8(out.stdout).unwrap();
        assert!(text.contains("6440円") && text.contains("大学生"));
        assert_eq!(text.contains("購入資格を保証しません"), !quiet);
    }
    let out = server.run(&["pass", "藤が丘", "名古屋", "--json", "--no-cache"]);
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(!text.contains("eligible") && !text.contains("購入資格"));
}

#[test]
fn delay_cert_filters_before_limit_and_known_lines_without_certificates_are_successful() {
    let server = Server::new(|_| {
        Response::json(
            r#"[
        {"delay_id":"1","rosen_name":"東山線","delay_datetime":"2026-09-08T01:00:00","title":"古い","max_delay_time":"60分以上"},
        {"delay_id":"2","rosen_name":"名城線・名港線","delay_datetime":"2026-09-15T00:00:00","title":"別路線","max_delay_time":"10分"},
        {"delay_id":"3","rosen_name":"東山線","delay_datetime":"2026-09-08T18:00:00","title":"同日の新しい証明書","max_delay_time":"15分"}
    ]"#,
        )
    });
    let out = server.run(&[
        "delay-cert",
        "--line",
        "東山",
        "--date",
        "2026-09-08",
        "--limit",
        "1",
        "--json",
    ]);
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["data"]["certificates"].as_array().unwrap().len(), 1);
    assert_eq!(v["data"]["certificates"][0]["title"], "同日の新しい証明書");
    for line in ["名城線", "名港線"] {
        let out = server.run(&["delay-cert", "--line", line, "--json"]);
        assert!(out.status.success());
        let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(v["data"]["certificates"].as_array().unwrap().len(), 1);
        assert_eq!(v["data"]["certificates"][0]["line"], "名城線・名港線");
    }
    let out = server.run(&["delay-cert", "--line", "鶴舞線", "--json"]);
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["complete"], true);
    assert_eq!(v["data"]["certificates"], serde_json::json!([]));
    for (line, code) in [("存在しない", "not_found"), ("線", "ambiguous")] {
        let out = server.run(&["delay-cert", "--line", line, "--json"]);
        assert_eq!(out.status.code(), Some(3));
        let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(v["errors"][0]["code"], code);
    }
    let server = Server::new(|_| Response::json("[]"));
    let out = server.run(&["delay-cert", "--line", "東山線"]);
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("該当する延着証明書はありません"));
    assert!(
        !text.contains("平常運行")
            && !text.contains("正常運行")
            && !text.contains("遅延していません")
    );
}

#[test]
fn delay_cert_invalid_dates_are_partial_even_when_all_certificates_are_invalid() {
    for date in ["not-a-date", "2026-02-30T00:00:00", ""] {
        let body = serde_json::json!([
            {"delay_id":"bad","rosen_name":"東山線","delay_datetime":date,"title":"不正","max_delay_time":"60分以上"},
            {"delay_id":"good","rosen_name":"東山線","delay_datetime":"2026-09-08T00:00:00","title":"有効","max_delay_time":"15分"}
        ]).to_string();
        let server = Server::new(move |_| Response::json(body.clone()));
        let out = server.run(&["delay-cert", "--json", "--quiet"]);
        assert_eq!(out.status.code(), Some(5));
        assert!(!out.stderr.is_empty());
        let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(v["complete"], false);
        assert_eq!(v["data"]["certificates"].as_array().unwrap().len(), 1);
        assert_eq!(v["data"]["certificates"][0]["title"], "有効");
        assert_eq!(v["errors"][0]["scope"], "delay-cert.certificates.0");
    }
    let server = Server::new(|_| {
        Response::json(
            r#"[{"delay_id":"bad","rosen_name":"東山線","title":"日時なし","max_delay_time":"60分以上"}]"#,
        )
    });
    let out = server.run(&["delay-cert", "--json"]);
    assert_eq!(out.status.code(), Some(5));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["data"]["certificates"], serde_json::json!([]));
    assert_eq!(v["complete"], false);
    assert_eq!(v["errors"][0]["code"], "invalid_response");
}

#[test]
fn delay_cert_never_persistently_caches_and_raw_keeps_the_original_body() {
    let body = "\u{feff}[\n{\"delay_id\":\"a&b\",\"rosen_name\":\"東山線\",\"delay_datetime\":\"2026-09-08T00:00:00\",\"title\":\"案内\",\"max_delay_time\":\"60分以上\"}\n]\n";
    let server =
        Server::new(move |_| Response::json(body).header("Cache-Control: max-age=86400\r\n"));
    let cache = std::env::temp_dir().join(format!("nkotsu-delay-cache-{}", std::process::id()));
    for _ in 0..2 {
        let out = Command::new(env!("CARGO_BIN_EXE_nkotsu"))
            .env("NKOTSU_BASE_URL", &server.url)
            .env("NKOTSU_CACHE_DIR", &cache)
            .args(["delay-cert", "--json"])
            .output()
            .unwrap();
        assert!(out.status.success());
        let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert!(
            v["data"]["certificates"][0]["url"]
                .as_str()
                .unwrap()
                .ends_with("delay_id=a%26b")
        );
    }
    assert_eq!(server.requests.lock().unwrap().len(), 2);
    assert!(!cache.exists());
    let out = server.run(&["delay-cert", "--raw"]);
    assert!(out.status.success());
    let raw: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        raw[format!("{}/datas/traffic_delay_certificate.json", server.url)],
        body
    );
}

#[test]
fn new_commands_reject_invalid_arguments_and_report_upstream_failures() {
    let server = Server::new(|_| panic!("invalid arguments must not fetch"));
    for args in [
        vec!["pass", "藤が丘", "名古屋", "--months", "2", "--json"],
        vec!["delay-cert", "--limit", "0", "--json"],
        vec!["delay-cert", "--date", "2026-02-30", "--json"],
        vec!["delay-cert", "--date", "2026-9-8", "--json"],
    ] {
        let out = server.run(&args);
        assert_eq!(out.status.code(), Some(2));
        let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(v["errors"][0]["code"], "invalid_arguments");
    }
    let server = Server::new(|_| Response {
        status: 400,
        mime: "application/json",
        body: "{}".into(),
        headers: String::new(),
    });
    for args in [
        vec!["fare", "藤が丘", "名古屋", "--json", "--no-cache"],
        vec!["pass", "藤が丘", "名古屋", "--json", "--no-cache"],
        vec!["delay-cert", "--json"],
    ] {
        let out = server.run(&args);
        assert_eq!(out.status.code(), Some(4));
        let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert!(v["data"].is_null());
        assert_eq!(v["errors"][0]["code"], "http_error");
    }
    let server = Server::new(|_| Response::json("null"));
    for args in [
        vec!["fare", "藤が丘", "名古屋", "--json", "--no-cache"],
        vec!["delay-cert", "--json"],
    ] {
        let out = server.run(&args);
        assert_eq!(out.status.code(), Some(5));
        let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert!(v["data"].is_null());
    }
}

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
fn whole_command_failure_is_json_with_a_stable_code() {
    let server = Server::new(|_| Response {
        status: 400,
        mime: "application/json",
        body: "{}".into(),
        headers: String::new(),
    });
    let out = server.run(&["--json", "--no-cache", "bus", "stop", "上社"]);
    assert_eq!(out.status.code(), Some(4));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["schema_version"], 1);
    assert_eq!(v["complete"], false);
    assert!(v["data"].is_null());
    assert_eq!(v["errors"][0]["code"], "http_error");
    assert_eq!(v["errors"][0]["scope"], "command");
    assert_eq!(v["errors"][0]["exit_code"], 4);
    assert!(!out.stderr.is_empty());
}

#[test]
fn argument_failures_use_json_but_help_and_version_stay_text() {
    for args in [
        vec!["--json", "unknown"],
        vec!["bus", "stop", "--json"],
        vec!["--raw", "status", "--json"],
        vec!["--json", "subway", "next", "藤が丘", "--limit", "0"],
        vec!["--json"],
    ] {
        let out = Command::new(env!("CARGO_BIN_EXE_nkotsu"))
            .args(args)
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(2));
        let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert!(v["data"].is_null());
        assert_eq!(v["errors"][0]["code"], "invalid_arguments");
        assert!(!out.stderr.is_empty());
    }
    for flag in ["--help", "--version"] {
        let out = Command::new(env!("CARGO_BIN_EXE_nkotsu"))
            .args(["--json", flag])
            .output()
            .unwrap();
        assert!(out.status.success());
        assert!(serde_json::from_slice::<serde_json::Value>(&out.stdout).is_err());
        assert!(out.stderr.is_empty());
    }
    let out = Command::new(env!("CARGO_BIN_EXE_nkotsu"))
        .args(["unknown", "--", "--json"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
}

#[test]
fn embedded_docs_work_without_network_or_cache() {
    let server = Server::new(|_| panic!("docs must not fetch an API"));
    let out = server.run(&["docs", "list", "--json"]);
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["complete"], true);
    let names: Vec<_> = v["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "bus",
            "subway",
            "route",
            "fare",
            "pass",
            "delay-cert",
            "places",
            "output",
            "troubleshooting"
        ]
    );
    for name in names {
        let out = server.run(&["docs", "show", name, "--json"]);
        assert!(out.status.success());
        let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(v["data"]["name"], name);
        assert!(!v["data"]["summary"].as_str().unwrap().is_empty());
        let text = v["data"]["content"].as_str().unwrap();
        assert!(text.starts_with("# "));
        let out = server.run(&["docs", "show", name]);
        assert!(out.status.success());
        assert_eq!(String::from_utf8(out.stdout).unwrap(), format!("{text}\n"));
    }
    for (args, code, exit) in [
        (vec!["docs", "show", "unknown", "--json"], "not_found", 3),
        (
            vec!["docs", "list", "--raw", "--json"],
            "invalid_arguments",
            2,
        ),
    ] {
        let out = server.run(&args);
        assert_eq!(out.status.code(), Some(exit));
        let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(v["errors"][0]["code"], code);
    }
    let out = server.run(&["docs", "show", "bus", "--raw"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
    assert!(server.requests.lock().unwrap().is_empty());
}

#[test]
fn help_explains_purpose_examples_constraints_and_docs() {
    for (args, required) in [
        (
            vec!["--help"],
            vec![
                "市バス・地下鉄情報を取得するCLI",
                "Examples:",
                "nkotsu route 藤が丘 名古屋 --subway",
                "nkotsu docs list",
                "nkotsu docs show <name>",
                "機械処理向けJSON",
            ],
        ),
        (
            vec!["bus", "--help"],
            vec![
                "市バスの情報を取得",
                "nkotsu bus stop 上社",
                "nkotsu bus live 上社",
            ],
        ),
        (
            vec!["bus", "live", "--help"],
            vec![
                "到着予測ではありません",
                "通過済みのバスも表示",
                "現在位置情報なし",
                "nkotsu docs show bus",
            ],
        ),
        (
            vec!["subway", "next", "--help"],
            vec![
                "予定列車",
                "リアルタイム",
                "翌営業日",
                "--limit <N>",
                "nkotsu docs show subway",
            ],
        ),
        (
            vec!["route", "--help"],
            vec![
                "今日",
                "翌日へ繰り越しません",
                "指定した時刻までに到着",
                "地下鉄のみ",
                "nkotsu docs show route",
            ],
        ),
        (
            vec!["fare", "--help"],
            vec![
                "複数",
                "最初の経路を最短",
                "--route <ROUTE>",
                "nkotsu docs show fare",
            ],
        ),
        (
            vec!["pass", "--help"],
            vec![
                "購入資格を判定しません",
                "--type <TYPE>",
                "--months <MONTHS>",
                "--with-bus",
                "nkotsu docs show pass",
            ],
        ),
        (
            vec!["search", "--help"],
            vec![
                "横断検索",
                "--type <TYPE>",
                "--limit <N>",
                "自動選択しません",
                "docs show places",
            ],
        ),
        (
            vec!["station", "--help"],
            vec!["駅記号", "docs show places"],
        ),
        (
            vec!["location", "--help"],
            vec!["現在位置", "--type <TYPE>", "docs show places"],
        ),
        (
            vec!["nearby", "--help"],
            vec![
                "概算直線距離",
                "--origin-type <TYPE>",
                "--radius <METERS>",
                "docs show places",
            ],
        ),
        (
            vec!["delay-cert", "--help"],
            vec![
                "現在の遅延状況ではありません。",
                "--line <LINE>",
                "--date <YYYY-MM-DD>",
                "--limit <N>",
                "nkotsu docs show delay-cert",
            ],
        ),
    ] {
        let out = Command::new(env!("CARGO_BIN_EXE_nkotsu"))
            .args(args)
            .output()
            .unwrap();
        assert!(out.status.success());
        assert!(out.stderr.is_empty());
        let text = String::from_utf8(out.stdout).unwrap();
        for phrase in required {
            assert!(text.contains(phrase), "missing {phrase}: {text}");
        }
    }
    for command in [
        vec!["status"],
        vec!["bus", "stop"],
        vec!["bus", "timetable"],
        vec!["subway"],
        vec!["subway", "timetable"],
        vec!["docs"],
        vec!["docs", "list"],
        vec!["docs", "show"],
        vec!["fare"],
        vec!["pass"],
        vec!["delay-cert"],
        vec!["search"],
        vec!["station"],
        vec!["location"],
        vec!["nearby"],
    ] {
        let out = Command::new(env!("CARGO_BIN_EXE_nkotsu"))
            .args(command)
            .arg("--help")
            .output()
            .unwrap();
        assert!(out.status.success());
        let text = String::from_utf8(out.stdout).unwrap();
        let examples = text
            .split("Examples:\n")
            .nth(1)
            .unwrap()
            .split("\n\n")
            .next()
            .unwrap();
        assert!(
            examples
                .lines()
                .filter(|line| line.trim().starts_with("nkotsu "))
                .count()
                >= 2,
            "{text}"
        );
        assert!(text.contains("nkotsu docs show "));
    }
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
                        stream.set_nonblocking(false).unwrap();
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
    assert_eq!(
        v["data"]["station"],
        serde_json::json!({"name":"藤が丘","type":"subway"})
    );
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
    assert_eq!(
        v["data"]["from"],
        serde_json::json!({"name":"藤が丘","type":"subway"})
    );
    assert_eq!(
        v["data"]["to"],
        serde_json::json!({"name":"名古屋","type":"subway"})
    );
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
    for (status, mime, body, code, requests, failure_code) in [
        (400, "application/json", "{}", 4, 1, "http_error"),
        (502, "application/json", "{}", 4, 3, "http_error"),
        (503, "application/json", "{}", 4, 3, "http_error"),
        (504, "application/json", "{}", 4, 3, "http_error"),
        (
            200,
            "text/html",
            "<html>maintenance</html>",
            5,
            1,
            "invalid_response",
        ),
        (
            200,
            "application/json",
            "<html>maintenance</html>",
            5,
            1,
            "invalid_response",
        ),
        (200, "application/json", "{invalid", 5, 1, "parse_error"),
    ] {
        let server = Server::new(move |_| Response {
            status,
            mime,
            body: body.into(),
            headers: String::new(),
        });
        let out = server.run(&["--no-cache", "--json", "status"]);
        assert_eq!(out.status.code(), Some(code));
        let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(
            v["errors"][0]["code"],
            failure_code,
            "status={status}: {}",
            String::from_utf8_lossy(&out.stderr)
        );

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
    let out = server.run(&["--no-cache", "--json", "route", "藤が丘", "藤が丘"]);
    assert_eq!(out.status.code(), Some(3));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["errors"][0]["code"], "ambiguous");
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
    assert_eq!(v["errors"][0]["code"], "http_error");
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
    for (xml, code, failure_code) in [
        (
            "<NorikaeData><Nstatus><Status>false</Status><Err><ErrorCode>NO_ROUTE</ErrorCode></Err></Nstatus></NorikaeData>",
            6,
            "upstream_error",
        ),
        (
            "<NorikaeData><Nstatus><Status>true</Status></Nstatus>",
            5,
            "parse_error",
        ),
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
        let out = server.run(&[
            "--no-cache",
            "--json",
            "route",
            "藤が丘",
            "藤が丘",
            "--subway",
        ]);
        let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(v["errors"][0]["code"], failure_code);
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

#[test]
fn connection_and_cache_failures_have_distinct_json_codes() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    let out = Command::new(env!("CARGO_BIN_EXE_nkotsu"))
        .env("NKOTSU_BASE_URL", url)
        .args(["--json", "--no-cache", "bus", "stop", "上社"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(4));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["errors"][0]["code"], "network_error");
    let server = Server::new(|_| panic!("cache read must fail before HTTP"));
    let out = Command::new(env!("CARGO_BIN_EXE_nkotsu"))
        .env("NKOTSU_BASE_URL", &server.url)
        .env("NKOTSU_CACHE_DIR", "/dev/null")
        .args(["--json", "bus", "stop", "上社"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["errors"][0]["code"], "cache_error");
    assert!(server.requests.lock().unwrap().is_empty());
}
