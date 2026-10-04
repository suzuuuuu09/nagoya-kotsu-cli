# Project knowledge

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
