# Project knowledge

**CI and Nix Rust versions**
CI and release workflows use the current Rust stable toolchain; the development shell follows the Rust version in `flake.lock`. They can differ. Rust 1.99 Clippy rejected the borrowed `stop_name` closure in bus live parsing while the Nix Rust 1.98 check passed. When CI lint fails, inspect its actual Rust version and validate the fix with that toolchain as well as the locked development shell.

**Cargo package include patterns**
Anchor release `include` patterns with a leading `/`. Unanchored names such as `README.md` and `LICENSE` also match files beneath `.direnv/flake-inputs`, and explicit includes override Git ignore rules. A local package check pulled Nixpkgs files into the archive until the patterns were rooted. Inspect the actual `.crate` file list, not just whether packaging compiles.

**Global output flag conflicts across subcommands**
Clap's global `conflicts_with` check does not reject every placement of output flags across command levels: `--raw status --json` can parse successfully. Check the parsed `json` and `raw` flags together before creating the API client, so incompatible output modes never trigger a request.

**Local HTTP test server sockets on macOS**
The nonblocking listener can yield nonblocking accepted streams on macOS. Explicitly set each accepted stream to blocking mode before reading a request. Otherwise the fixture's read loop can treat a would-block read as EOF and send a response to an empty request, intermittently turning expected HTTP failures into Hyper `UnexpectedMessage` network errors. HTTP tests became repeatable after this change; production HTTP behavior does not need a workaround.

**Route endpoint names and transport modes**
The official `Suggest/StationInfos/json` response separates `station_name` from `station_div`. The official site's `GetNameFromItem` adds transport-specific suffixes for display; those suffixes are not part of the API's `station_name`. Exact names can identify different transport facilities: `藤が丘` has city bus and subway candidates, and `名古屋` has subway and Aonami Line candidates. Preserve the transport mode when resolving names and constructing the search name.

*Avoid*: Assuming an exact name is unique, merging facilities solely by name, or treating a displayed transport suffix as part of every upstream name field.

Sources: [StationInfos](https://map.kotsu.city.nagoya.jp/optimizeapi/api/Suggest/StationInfos/json?key=%E8%97%A4%E3%81%8C%E4%B8%98&marge=pole), [suggest display logic](https://map.kotsu.city.nagoya.jp/rp/Jassets/lib/js_suggest/js_suggest.js), [route search logic](https://map.kotsu.city.nagoya.jp/rp/Jassets/js/route/JNRMForOSM.js).

**Active timetable masters**
The current bus and subway frontend's `GetStationDataUrl` uses `/station_data/`, even though settings also contain legacy uppercase `/STATION_DATA/` paths. The subway timetable master supplies its own `id`, distinct from the suggest endpoint's `station_cd` and `station_num`. Resolve those masters separately. The bus `station_infos/station_name.json` currently encodes IDs as JSON numeric tokens; preserve their lexical text as strings without numeric conversion.

Sources: [bus station frontend](https://www.kotsu.city.nagoya.jp/rp/Jassets/js/bus/station.js), [subway frontend](https://www.kotsu.city.nagoya.jp/rp/Jassets/js/subway/subway.js).

**Route segment transport classification**
The route XML can report `KEIRONAME=バス` and an empty `LINETYPE` for actual subway segments. Determine subway membership from the confirmed subway `ROSENNAME` values, and bus membership from explicitly bus-marked endpoint names; do not use `KEIRONAME` alone to enforce CLI transport filters. `Nstatus/Status` is the boolean text `true` on a successful response.

Source: official `GetLatitudeLongitudePlusMultiIOSearchRouteDiagram` response for 藤が丘 to 名古屋, using subway endpoints and `use_agency=名古屋市地下鉄`.
