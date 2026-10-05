# CLI動作仕様

## 対象コマンド

実装言語はRust、Cargo packageは `nagoya-kotsu-cli`、バイナリ名は `nkotsu` とする。v0.2.0は `status`、`bus stop`、`bus timetable`、`bus live`、`subway timetable`、`subway next`、`route`、`fare`、`pass`、`delay-cert` を対象とする。

v0.1.0の既存コマンド・JSON構造・終了コードは維持する。追加機能の仕様は [普通運賃](cli/fare.md)、[定期券料金](cli/pass.md)、[延着証明書](cli/delay-cert.md) に記録している。

## 設計上の決定

- [日種の自動選択](adr/0001-select-timetable-day-types.md): 祝日を含めて判定し、適用を確認できた特別ダイヤを優先する。判断できない場合は指定を求める。
- [営業日と次発案内](adr/0002-use-service-days-for-timetables.md): 午前4時で区切り、次発案内は翌営業日まで検索する。`--day` は対象営業日だけに適用する。
- [名前解決と経路の交通手段](adr/0003-reject-ambiguous-endpoint-names.md): 曖昧な名前は選択しない。交通手段指定を地点候補と乗車区間の両方に適用する。
- [部分失敗](adr/0004-report-partial-results-as-errors.md): 得られた結果と欠落を返し、非0で終了する。
- [正規化済みJSON](adr/0005-wrap-normalized-json-results.md): `schema_version`、`complete`、`data`、`errors` の共通形式を使う。

用語はルートの [GLOSSARY.md](../GLOSSARY.md) に従う。

## JSONエラー

`--json` 指定時は、コマンド全体の失敗も `schema_version: 1`、`complete: false`、`data: null`、`errors` の共通形式でstdoutへ出力する。人間向けエラー文をstdoutへ混在させない。引数解析の失敗（不明なコマンド、必須引数不足、オプション競合を含む）も対象とし、機械判定用の `code` は `invalid_arguments`、終了コードは従来どおり2とする。`--json --raw` の競合もJSONエラーとして返す。

`--help` と `--version` は失敗ではなく、`--json` 指定時も従来の表示を維持する。

`Failure` に機械判定用の `code` を追加する。対応は以下とし、既存の終了コード0〜6は変更しない。全体失敗時も部分失敗時も同じ分類を使う。全体失敗の `scope` は `command` とし、部分失敗の既存の `scope` は維持する。

| エラー種別 | code | 終了コード |
| --- | --- | --- |
| 引数 | `invalid_arguments` | 2 |
| 未発見 | `not_found` | 3 |
| 曖昧 | `ambiguous` | 3 |
| ネットワーク | `network_error` | 4 |
| HTTP | `http_error` | 4 |
| 不正なレスポンス | `invalid_response` | 5 |
| 解析 | `parse_error` | 5 |
| API内部エラー | `upstream_error` | 6 |
| キャッシュ | `cache_error` | 1 |

`--json` 時も人間向けのエラー・警告、`--verbose` の詳細はstderrへ出す。stdoutにはJSONだけを出す。

## 内蔵ドキュメント

`docs list` と `docs show <name>` を追加し、`bus`、`subway`、`route`、`fare`、`pass`、`delay-cert`、`output`、`troubleshooting` の8文書をバイナリへ埋め込む。インストール後はCLI単体で参照できる。開発Agent向けの `AGENTS.md` と `docs/agents/` の役割は変更しない。

通常の `docs list` は名前と概要、`docs show` はMarkdown本文をstdoutへ出力する。`--json` は既存Envelopeを使い、`list` の `data` は名前・概要の一覧、`show` の `data` は名前・概要・Markdown本文とする。API本文を取得しないため、`docs --raw` は `invalid_arguments`、終了コード2で拒否する。

文書名は上記の名前の完全一致で選択し、未知の名前は `not_found`、終了コード3で利用可能な名前を案内する。文書の参照ではAPI・永続キャッシュを利用しない。トップレベルのhelpには `nkotsu docs list` と `nkotsu docs show <name>` の案内を追加する。

既存の交通情報のDomain Modelと `--raw` の形式は維持する。`--format`、MCP、全文検索、JSON Schema生成は追加しない。CLI利用者向けSkillは既存のhelp・内蔵docsへ誘導する。

## Help表示

Clapの自動生成helpを基本とし、`about`、`long_about`、`after_help` で目的、代表例、重要な挙動を補足する。説明は内部実装ではなく、指定すると利用者にとって何が変わるかを記す。

各コマンドのhelpは、目的、Usage、Arguments、Options、Examples、必要な場合のみNotes、詳細ドキュメントへの導線の順を基本とする。各主要コマンドに最低2つの実行例を載せ、内蔵ドキュメントの重要な制約も1〜2行で要約する。既存のオプションを省略せず、トップレベルにグローバルオプションの説明と `docs list` / `docs show <name>` の案内を載せる。

`bus` は停留所・時刻表・接近情報を取得することを説明する。`bus live` は到着予測ではなく、現在位置情報と通過履歴を表示すること、`--all` は通過済み車両も含めること、履歴から現在位置・到着予測・遅延分・GPS座標を推定しないことを説明する。

`subway next` は時刻表に基づく予定列車であり、リアルタイムの列車位置ではないことを説明する。`route` は `--arrive` が指定時刻までに到着する経路を検索すること、`--at HH:MM` が日本時間の今日を使い、過去時刻を翌日へ繰り越さないことを説明する。

トップレベルと `route` の代表例で `藤が丘` から `名古屋` を検索する場合は、`--subway` を付ける。例をそのまま実行しても交通施設の曖昧性で失敗しないよう、[ADR-0003](adr/0003-reject-ambiguous-endpoint-names.md) に揃える。`--bus` だけなら市バスのみ、`--subway` だけなら地下鉄のみ、無指定または両方指定なら両方を対象にすること、交通手段指定が地点候補と乗車区間の両方へ適用されることも短く説明する。

Helpの追加によって引数解析・名前解決・検索条件の既存の挙動は変更しない。

## バス接近情報

`bus live` は、通常は対象停留所を未通過のバスと、対象停留所との位置関係が不明なバスを表示する。`--all` を指定すると、対象停留所を通過済みと確認できるバスも表示する。`--all` を指定しても、`--route` と `--pole` の絞り込みは維持する。

現在位置がなく通過履歴しか得られない場合は、通常表示でも「現在位置情報なし」と最終通過を表示する。履歴から現在位置や対象停留所との位置関係を推定しない。

## 絞り込みと0件の区別

系統・のりばなどの絞り込み指定が存在しない場合は、候補を示して終了コード3で終了する。有効な指定で対象の便などが0件だった場合は、該当データがないことを表示し、終了コード0で終了する。入力ミスを実際の0件と取り違えないようにする。

## キャッシュ再取得の失敗

静的マスターや時刻表の再取得・再検証に失敗した場合は、期限切れキャッシュへフォールバックせず、失敗の分類に対応するエラーで終了する。`--refresh` による取得に失敗した場合も、保存済みの古いデータへ戻さない。

## 複数APIのraw出力

複数APIを使うコマンドの `--raw` は、APIごとにキーを分けたJSONオブジェクトで返す。JSON・XMLとも、レスポンス本文を文字列として格納し、本文の空白・改行・BOMを保持する。JSON本文もJSON値へ再解析して出力しない。

外側のJSONでは本文を文字列としてエスケープする。デバッグ時はその文字列を取り出して元の本文を確認し、JSONデータを操作する場合は必要に応じて再解析する。

## quiet出力

`--quiet` は進捗などの補助通知だけを抑制し、取得結果のstdoutは維持する。`--quiet --json` または `--quiet --raw` の場合も、それぞれのデータをstdoutへ出力する。エラーや、取得結果の欠落を知らせる警告は表示する。`--json` と `--raw` の同時指定は許可しない。

## 経路検索の日付省略

経路検索は常に `Asia/Tokyo` の暦日時で扱う。`route --at HH:MM` のように日付を省略した場合は、日本時間の今日の日付を使う。指定時刻が現在時刻より前でも、翌日へ繰り越さない。日付を含む指定では、その日付を使う。

例えば10月5日の23時に `route --at 01:00` を実行した場合は、10月5日の01時として検索する。翌日を検索したい場合は、日付も指定する。時刻表と次発案内の営業日境界を、経路検索の日付補完には適用しない。検索条件の指定がなければ、現在時刻からの出発検索とする。
