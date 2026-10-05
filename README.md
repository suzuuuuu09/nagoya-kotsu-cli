# nagoya-kotsu-cli

名古屋市交通局の市バス・地下鉄情報を取得するRust製CLIです。バイナリ名は `nkotsu` で、v0.3.0では運行情報、停留所、時刻表、接近情報、経路検索、普通運賃、定期券料金、延着証明書に加え、駅・バス停の検索、地下鉄駅情報、座標、周辺検索を利用できます。

## インストール

crates.ioで公開された版は、Rustツールチェーンで次のようにインストールできます。

```sh
cargo install nagoya-kotsu-cli --locked
```

Rust環境を使わない場合は、[GitHub Releases](https://github.com/suzuuuuu09/nagoya-kotsu-cli/releases)に公開されたOS・CPU別のアーカイブを展開し、`nkotsu`（Windowsでは `nkotsu.exe`）をPATHが通ったディレクトリへ配置してください。Linux・macOSは `.tar.gz`、Windowsは `.zip` です。Linux版はUbuntu 24.04上でビルドしたGNU版を配布します。

公開前のコードやローカルの変更を試す場合は、リポジトリのルートでインストールしてください。

```sh
cargo install --path . --locked
```

## コマンド別の使い方

### 運行情報: `status`

市バス・地下鉄の運行状況や運行変更の記事を表示します。路線を指定するには `--line`、市バスまたは地下鉄に絞るには `--bus` または `--subway` を使います。交通手段の両方を指定すると、絞り込みません。

```sh
nkotsu status
nkotsu status --line 東山線
nkotsu status --bus --json
```

記事作成日時とAPI取得日時は別の項目です。有効な路線で記事が0件の場合も成功として扱い、取得に失敗した場合はエラーを返します。

### 停留所とのりば: `bus stop`

バス停名を指定し、停留所IDとのりばを表示します。時刻表や接近情報をのりばで絞り込む前に、利用するのりば名を確認できます。

```sh
nkotsu bus stop 上社
nkotsu bus stop 上社 --json
```

### 市バスの時刻表: `bus timetable`

指定した停留所の発車予定を表示します。系統・のりば・時刻を絞り込み、表示件数を指定できます。

```sh
nkotsu bus timetable 上社
nkotsu bus timetable 上社 --pole 4番 --route 上社12 --after 14:00 --limit 10
nkotsu bus timetable 上社 --day weekday --json
```

| オプション | 指定すると変わる内容 |
| --- | --- |
| `--route <ROUTE>` | 系統名で絞り込む。例: `上社12` |
| `--pole <POLE>` | のりばで絞り込む。例: `4番` |
| `--day <DAY>` | 使用する日種を指定する。省略時は自動判定 |
| `--after <HH:MM>` | 指定時刻以降の予定便を表示する。00:00〜27:59を指定可能 |
| `--limit <N>` | 表示件数を1件以上で指定する |

存在しない系統・のりばの指定はエラーになります。有効な指定で便が0件の場合は成功として扱います。営業日と日種の扱いは、後述の「時刻表の営業日と日種」を参照してください。

### 市バスの接近情報: `bus live`

指定した停留所の現在位置情報と通過履歴を表示します。系統やのりばで絞るには、`--route` と `--pole` を使います。

```sh
nkotsu bus live 上社 --route 上社12
nkotsu bus live 上社 --pole 4番
nkotsu bus live 上社 --route 上社12 --all --json
```

`--all` は対象停留所を通過済みの車両も表示するオプションです。指定しても系統・のりばの絞り込みは維持します。表示対象と、現在位置が取得できない場合の扱いは、後述の「接近情報の表示範囲」を参照してください。

### 地下鉄の時刻表: `subway timetable`

地下鉄駅の発車予定を表示します。路線を指定するには `--line`、方面を指定するには `--direction` を使います。

```sh
nkotsu subway timetable 藤が丘 --line 東山線
nkotsu subway timetable 藤が丘 --day weekday --after 14:00 --limit 10
nkotsu subway timetable 栄 --line 東山線 --direction 藤が丘方面 --json
```

`--day` は使用する日種、`--after HH:MM` は表示を始める時刻、`--limit N` は表示件数を指定します。時刻には00:00〜27:59、件数には1以上を指定できます。日種を自動判定できない場合は、`--day` を指定してください。

### 地下鉄の次発案内: `subway next`

指定時刻以降に発車する予定列車を、時刻表から検索します。`--at` を省略すると現在時刻を使い、`--limit` を省略すると5件表示します。

```sh
nkotsu subway next 藤が丘 --limit 3
nkotsu subway next 栄 --line 東山線 --direction 藤が丘方面
nkotsu subway next 藤が丘 --at 23:59 --day weekday --limit 3 --json
```

路線・方面・日種は、時刻表と同じ `--line`、`--direction`、`--day` で指定できます。`--at HH:MM` には00:00〜27:59を指定できます。検索範囲と日種の上書きは、後述の「時刻表の営業日と日種」を参照してください。

### 経路検索: `route`

出発地と到着地を指定し、経路を検索します。地下鉄駅の「藤が丘」から「名古屋」を調べる例では、同名の市バス停と区別するために `--subway` を指定しています。

```sh
nkotsu route 藤が丘 名古屋 --subway --at 09:00
nkotsu route 藤が丘 名古屋 --subway --via 栄 --arrive --at 18:00 --details
nkotsu route 藤が丘 名古屋 --subway --first
nkotsu route 藤が丘 名古屋 --subway --last --slow-transfer --json
```

| オプション | 指定すると変わる内容 |
| --- | --- |
| `--at <TIME>` | 検索日時を指定する。`HH:MM` または `YYYY-MM-DDTHH:MM` |
| `--arrive` | 指定時刻までに到着する経路を検索する |
| `--first` | 始発を検索する |
| `--last` | 終発を検索する |
| `--via <STATION>` | 経由地を指定する |
| `--bus` | 市バスのみを対象にする。`--subway` も指定すると両方 |
| `--subway` | 地下鉄のみを対象にする。`--bus` も指定すると両方 |
| `--slow-transfer` | ゆっくり乗換の条件で検索する |
| `--details` | のりば・区間運賃などの詳細を表示する |

`--first` と `--last` は互いに同時指定できず、どちらも `--at` や `--arrive` と組み合わせることはできません。日付を省略した場合の扱いは、後述の「経路検索の日時」を参照してください。

### 普通運賃: `fare`

料金検索専用の駅マスターと経路データから、駅間の普通運賃を取得します。既定では料金経路をすべて返します。掲載順だけでは最短・推奨とする根拠にならないため、先頭の経路をそのようには扱いません。

```sh
nkotsu fare 藤が丘 名古屋
nkotsu fare 藤が丘 名古屋 --route 東山線 --json
```

`--route` を指定すると、料金経路名で絞り込みます。両駅が存在しても料金経路がなければ、正常な0件として終了します。

欠落・不正な運賃はJSONで `null` とし、経路と正常な運賃を残して部分失敗を返します。検索と出力の詳細は `nkotsu docs show fare` を参照してください。

### 定期券料金: `pass`

料金経路ごとの定期券料金を取得します。利用者の通学証明や発売条件を確認しないため、表示料金は購入資格を保証しません。

```sh
nkotsu pass 藤が丘 名古屋
nkotsu pass 藤が丘 名古屋 --type 大学生 --months 1
nkotsu pass 藤が丘 名古屋 --with-bus --json
```

`--route` は経路、`--type` はAPIの券種名、`--months` は1・3・6か月の期間を指定します。券種名の候補は全経路から集め、一度だけ検索します。`--with-bus` は市バス併用用区分を使います。対応がない場合に通常区分へ戻すと、指定条件と異なる料金になるため、通常区分には戻しません。

一部の取得失敗や料金の欠落があっても、正常な料金と経路を保持し、失敗した範囲を返します。検索と部分失敗の詳細は `nkotsu docs show pass` を参照してください。

### 延着証明書: `delay-cert`

公開済みの延着証明書一覧を取得します。証明書は過去の遅延を証明するものであり、現在の遅延状況ではありません。現在の運行情報は `nkotsu status` で確認してください。

```sh
nkotsu delay-cert
nkotsu delay-cert --line 東山線
nkotsu delay-cert --date 2026-09-08 --limit 5 --json
```

`--line` は路線、`--date` は証明対象の暦日、`--limit` は1件以上の表示件数を指定します。既定では全件を証明対象日時の新しい順に返します。有効な路線で証明書が0件の場合も成功です。

一覧に現在の運行情報は含まれないため、証明書が0件でも現在の正常運行を意味しません。検索と出力の詳細は `nkotsu docs show delay-cert` を参照してください。

### 駅・バス停検索: `search`

市バス停と地下鉄駅を横断検索します。`--type` で種別、`--limit` で件数を指定できます。候補は自動選択せず、返された `qualified_name` で同名施設を区別します。

```sh
nkotsu search 藤が丘
nkotsu search 藤が丘 --type subway --limit 5 --json
```

### 地下鉄駅情報: `station`

地下鉄駅の名前・駅記号・代表座標を表示します。座標が取得できない場合も、正常な駅情報を保持します。

```sh
nkotsu station 藤が丘
nkotsu station 名古屋 --json
```

### 座標: `location`

交通施設の代表座標を取得します。利用者や端末の現在位置は取得しません。同名施設は完全修飾名か `--type` で指定します。

```sh
nkotsu location '藤が丘(名古屋市地下鉄)'
nkotsu location 上社 --type bus --json
```

### 周辺検索: `nearby`

指定施設から近い駅・バス停を検索します。`--origin-type` は基準地点、`--type` は結果の種別を指定します。距離は概算直線距離であり、徒歩距離・徒歩時間ではありません。

```sh
nkotsu nearby 藤が丘 --origin-type subway --type bus
nkotsu nearby 栄 --origin-type subway --radius 500 --limit 5 --json
```

詳しい検索・座標検証・部分失敗の仕様は `nkotsu docs show places` を参照してください。

### 名前の検索と候補の選択

名前はNFKC正規化し、完全一致する候補を優先して検索します。完全一致する候補がなければ部分一致で検索し、候補が複数あれば候補を表示して終了します。

経路検索では、`--bus` だけなら市バスのみ、`--subway` だけなら地下鉄のみを対象にします。両方を指定した場合と、どちらも指定しない場合は、市バスと地下鉄の両方を対象にします。同名の交通施設を区別するには、交通手段を指定するか、`藤が丘(名古屋市地下鉄)` のような具体的な候補名を使ってください。

## ヘルプと内蔵ドキュメント

コマンドごとの目的、実行例、全オプションは `nkotsu <コマンド> --help` で確認できます。詳細ドキュメントもバイナリに内蔵しているため、インストール後に通信なしで参照できます。

```sh
nkotsu docs list
nkotsu docs list --json
nkotsu docs show bus
nkotsu docs show output --json
```

`docs list` は文書名と概要を一覧表示し、`docs show <name>` は指定した文書のMarkdown本文を表示します。文書名は `bus`、`subway`、`route`、`fare`、`pass`、`delay-cert`、`places`、`output`、`troubleshooting` です。

`--json` を指定すると、`list` は名前・概要の一覧、`show` は名前・概要・本文をJSONで返します。APIの生レスポンスを取得するコマンドではないため、docsでの `--raw` 指定は引数エラーになります。

開発Agent向けの `AGENTS.md` と `docs/agents/` は、リポジトリで作業する際の指示です。CLIの使い方は、上記の内蔵ドキュメントで確認してください。

CLIを利用するエージェント向けに、[nagoya-kotsu skill](skills/nagoya-kotsu/SKILL.md)を用意しています。`skills/nagoya-kotsu/` を利用するエージェントのskillディレクトリへ配置してください。実行には、PATH上の `nkotsu` が必要です。skillは問い合わせに応じたコマンドの選択と結果の読み方を案内し、詳しい仕様はインストールされた版の内蔵ドキュメントで確認します。

## 時刻表の営業日と日種

時刻表と接近情報は04:00を営業日の境界とし、00〜03時台を前の営業日の24〜27時台として表示します。

地下鉄の次発案内は時刻表から算出した予定列車であり、実際の列車位置を示しません。翌営業日まで検索して各便の日付と日種を表示し、`--day` の上書きは最初の営業日だけに適用します。

日種は、時刻表を分類する運行日の区分です。`--day` に指定できる値は次のとおりです。

- 市バス: `weekday` / `saturday` / `holiday`
- 地下鉄: `weekday` / `holiday` / `new-year` / `all-night`

日種の自動選択には、同梱の内閣府祝日データと、公式サイト設定に明記された適用期間・特別休日を使います。特別ダイヤのデータが存在するだけでは、そのダイヤを選択しません。祝日データの対象年は1955〜2027年で、対象外の年では `--day` を指定してください。臨時変更をすべて自動判定することは保証していません。

## 経路検索の日時

経路検索の日時は日本時間として扱います。`--at HH:MM` は今日の日付、`--at 2026-10-05T09:00` は指定日として検索し、過去の時刻でも翌日へ繰り越しません。

## 接近情報の表示範囲

市バスの接近情報は、取得した現在位置情報と通過履歴を表示します。通常は対象停留所を未通過の車両と、対象停留所との位置関係が不明な車両を表示し、`--all` を指定すると通過済みの車両も含めます。

通過履歴だけの場合は「現在位置情報なし」と表示します。履歴が示すのは記録された通過地点と時刻であり、取得時点の位置を保証しません。そのため、履歴から現在位置、到着予測、遅延分、GPS座標を推定しません。

## 出力形式と終了コード

出力やキャッシュに関するオプションは全コマンド共通です。

取得結果は標準出力（stdout）へ、エラー、取得結果の欠落を知らせる警告、診断情報は標準エラー出力（stderr）へ出します。`--quiet` は補助メッセージだけを抑制し、結果・エラー・警告は残します。`--verbose` は詳細な診断を表示します。

表示にはANSIカラーを使用せず、`--no-color` も指定できます。

`--json` は、コマンドごとの正規化済みデータを共通の外側のオブジェクト（Envelope）に格納します。成功時の形式は次のとおりです。

```json
{"schema_version":1,"complete":true,"data":{},"errors":[]}
```

独立した取得の一部が失敗した場合は、取得できたデータを保持し、`complete: false` と `errors` を返します。引数解析を含むコマンド全体の失敗もJSONで返し、その場合は `data: null` とします。どちらの失敗も終了コードは非0で、stdoutに人間向けエラーを混在させません。

各エラーには `scope`、`code`、`exit_code`、`message` が含まれます。機械的に種別を判定する場合は、メッセージの文字列ではなく `code` を使ってください。終了コード0は成功を示し、有効な条件で結果が0件の場合も含みます。非0の終了コードとエラー種別の対応は次のとおりです。

| 終了コード | エラー種別 | code |
| --- | --- | --- |
| 1 | その他。キャッシュの失敗を含む | `cache_error`（キャッシュの失敗時） |
| 2 | 引数 | `invalid_arguments` |
| 3 | 未発見・曖昧 | `not_found` / `ambiguous` |
| 4 | HTTP・ネットワーク | `http_error` / `network_error` |
| 5 | レスポンス形式・解析 | `invalid_response` / `parse_error` |
| 6 | API内部エラー | `upstream_error` |

一方、`--raw` は取得URLをキー、元のレスポンス本文を値とするJSONオブジェクトを返します。本文は文字列として格納し、BOM、空白、改行を保持します。`--json` と `--raw` は同時指定できません。

`--help` と `--version` は、`--json` を指定しても通常の文字表示で返します。

## キャッシュとHTTP取得

静的マスター・料金データと時刻表は、OS標準のユーザーキャッシュディレクトリへ保存します。HTTPのCache-Control、ETag、Last-Modifiedを使い、期限の指定がなければ有効期間をマスターは24時間、時刻表は6時間とします。運行情報・接近情報・延着証明書は毎回取得します。

保存済みのデータを再検証するには `--refresh` を指定します。`--no-cache` は、永続キャッシュの読み書きを無効にします。再取得に失敗しても、古いキャッシュへ戻ることはありません。

HTTPタイムアウトは `--timeout SECONDS` で指定でき、既定値は10秒です。同時HTTP数は最大4で、接続失敗、タイムアウト、HTTP 502・503・504の場合に限り、最大2回再試行します。

## 開発環境と検証

Nixとdirenvがある場合は、初回に `direnv allow` を実行するとRustの開発ツールが読み込まれます。direnvを使わない場合は `nix develop` で同じ環境に入れます。依存するNixpkgsの版は `flake.lock` で固定しています。

フォーマットと検証には、次のコマンドを使います。

```sh
nix fmt flake.nix
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --locked
python3 -m unittest discover -s tests -p release.py
```

テストはローカルHTTPサーバーを使い、実際のCLIのstdout、stderr、終了コードを確認します。公式APIにはアクセスしません。テスト・開発用の `NKOTSU_BASE_URL` は全APIの取得先、`NKOTSU_CACHE_DIR` はキャッシュ保存先を上書きします。

リリース補助処理にはPython 3.11以降を使います。Nixの開発環境にもPythonを含めています。タグによる公開と初回認証の設定は、[リリース手順](docs/release.md)を参照してください。

## 祝日データの更新

祝日データは、[内閣府の国民の祝日](https://www8.cao.go.jp/chosei/shukujitsu/gaiyou.html)の[CSV](https://www8.cao.go.jp/chosei/shukujitsu/syukujitsu.csv)から日付列を取り出し、UTF-8の `YYYY-MM-DD` に変換したものです。更新時は公式CSVの対象年を確認し、`data/holidays.txt` と、このREADMEに記した対象年を合わせて更新してください。

## 内部APIと設計資料

公式サイトの内部APIを利用しており、公開APIとしての互換性保証はありません。実装上の決定は [CLI動作仕様](docs/cli-behavior.md)、用語は [GLOSSARY.md](GLOSSARY.md)、確認したAPI固有の注意点は [KNOWLEDGE.md](KNOWLEDGE.md) に記録しています。

## ライセンス

プロジェクトのコードは [Apache License 2.0](LICENSE) で公開します。同梱の祝日データの出典・加工内容・利用条件は [NOTICE](NOTICE) に記載しています。依存ライブラリにはそれぞれのライセンスが適用され、配布バイナリにはライセンス本文を収録した `THIRD-PARTY-LICENSES.json` を同梱します。
