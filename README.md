# nagoya-kotsu-cli

名古屋市交通局の市バス・地下鉄情報を取得するRust製CLI。バイナリ名は `nkotsu` です。v0.1.0の対象は運行情報、停留所、時刻表、接近情報、経路検索です。

## インストール

Rustツールチェーンを用意し、このディレクトリで実行します。

```sh
cargo install --path . --locked
```

## 使用例

```sh
nkotsu status
nkotsu bus stop 上社
nkotsu bus timetable 上社 --pole 4番 --route 上社12 --after 14:00 --limit 10
nkotsu bus live 上社 --route 上社12
nkotsu subway timetable 藤が丘 --line 東山線
nkotsu subway next 藤が丘 --limit 3
nkotsu route 藤が丘 名古屋 --subway --at 09:00
nkotsu route 藤が丘 名古屋 --subway --via 栄 --arrive --at 18:00 --details
```

名前をNFKC正規化して完全一致、部分一致の順に検索します。同じ名前のバス停・地下鉄駅などが複数ある場合は、候補を表示して終了します。経路検索では `--bus` / `--subway`、または `藤が丘(名古屋市地下鉄)` のような具体的な候補名を指定できます。両フラグ指定・無指定では市バスと地下鉄の両方を利用します。

コマンドごとの全オプションは `nkotsu <コマンド> --help` で確認できます。

## 時刻・日種

時刻表・接近情報は04:00を営業日の境界とし、00〜03時を24〜27時として表示します。次発は時刻表から算出した**予定列車**です。翌営業日まで調べ、各便の日付・日種を表示します。`--day` は最初の営業日だけに適用します。

- 市バスの `--day`: `weekday` / `saturday` / `holiday`
- 地下鉄の `--day`: `weekday` / `holiday` / `new-year` / `all-night`

自動選択は同梱の内閣府祝日データと、公式サイト設定に明記された適用期間・特別休日に基づきます。ダイヤのキーがあるだけでは特別ダイヤを選びません。祝日データの対象年は1955〜2027年で、対象外は `--day` を指定してください。臨時変更すべての自動判定は保証しません。

経路検索は常に日本時間です。`--at HH:MM` は今日の暦日、`--at 2026-10-05T09:00` は指定日として検索します。過去時刻を翌日へ繰り越しません。`--first` は始発、`--last` は終発、`--slow-transfer` はゆっくり乗換です。

接近情報の通過履歴から現在位置・到着予測・遅延分・GPS座標を推定しません。履歴だけなら「現在位置情報なし」と表示します。通常は未通過・位置関係不明の車両を表示し、`--all` は通過済み車両も含めます。

## 出力とキャッシュ

全コマンドで `--json` / `--raw` / `--refresh` / `--no-cache` / `--timeout SECONDS` / `--verbose` / `--quiet` / `--no-color` が使えます。デフォルトタイムアウトは10秒、表示はANSIカラーを使用しません。`--quiet` でも結果・エラー・欠落の警告は残します。

`--json` はAPIの生データではなく、正規化した安定形式です。

```json
{"schema_version":1,"complete":true,"data":{},"errors":[]}
```

一部取得に失敗すると、取得済みデータを残して `complete: false` と `errors` を返し、非0で終了します。`--raw` はURLごとのオブジェクトに元のレスポンス本文を文字列で格納し、BOM・空白・改行を保持します。`--json` と同時指定はできません。

静的マスター・時刻表をOS標準のユーザーキャッシュディレクトリに保存します。HTTPのCache-Control、ETag、Last-Modifiedを使い、期限指定がない場合はマスター24時間・時刻表6時間です。`--refresh` は再検証、`--no-cache` は永続キャッシュの読み書きを無効にします。再取得に失敗しても古いキャッシュへ戻りません。運行情報・接近情報は毎回取得します。同時HTTP数は最大4、接続失敗・タイムアウト・502/503/504だけ最大2回再試行します。

終了コード: 0成功（0件を含む）、1その他、2引数、3未発見・曖昧、4HTTP・ネットワーク、5レスポンス形式・解析、6API内部エラー。

## 開発・検証

Nixとdirenvがある場合は、初回に `direnv allow` を実行するとRustの開発ツールが読み込まれます。direnvを使わない場合は `nix develop` で同じ環境に入れます。依存するNixpkgsの版は `flake.lock` で固定しています。

```sh
nix fmt flake.nix
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --locked
```

テストはローカルHTTPサーバーを使い、実際のCLIのstdout・stderr・終了コードを確認します。公式APIへアクセスしません。テスト・開発用の `NKOTSU_BASE_URL` は全APIの取得先、`NKOTSU_CACHE_DIR` はキャッシュ保存先を上書きします。

祝日データは[内閣府の国民の祝日](https://www8.cao.go.jp/chosei/shukujitsu/gaiyou.html)の[CSV](https://www8.cao.go.jp/chosei/shukujitsu/syukujitsu.csv)から日付列をUTF-8の `YYYY-MM-DD` に変換したものです。更新時は公式CSVの対象年を確認し、`data/holidays.txt` と上記対象年を合わせて更新してください。

公式サイトの内部APIを利用しており、公開APIとしての互換性保証はありません。実装上の決定は [CLI動作仕様](docs/cli-behavior.md)、用語は [GLOSSARY.md](GLOSSARY.md)、確認したAPI固有の注意点は [KNOWLEDGE.md](KNOWLEDGE.md) を参照してください。普通運賃・定期券・延着証明書は次の段階の対象です。
