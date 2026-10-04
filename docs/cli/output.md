# 出力・終了コード・キャッシュ

通常のstdoutは人間向けの結果です。エラー・欠落の警告・診断はstderrへ出します。`--quiet` は補助メッセージだけを抑制し、結果と警告は残します。`--verbose` はHTTP取得やエラーの詳細をstderrへ出します。両者は同時指定できません。表示はANSIカラーを使用せず、`--no-color` も指定できます。

```sh
nkotsu status --json
nkotsu bus stop 上社 --raw
nkotsu docs list --json
nkotsu docs show route --json
```

`--json` は正規化したJSONを共通Envelopeで返します。

```json
{"schema_version":1,"complete":true,"data":{},"errors":[]}
```

独立した取得の部分失敗は成功データを保持し、`complete: false` と `errors` を返します。引数解析を含む全体失敗は `data: null` とし、非0で終了します。各エラーは `scope`、`code`、`exit_code`、`message` を持ちます。全体失敗の `scope` は `command`。詳細診断はJSONに含めずstderrへ出します。messageから種別を判定せずcodeを使用してください。

| code | 終了コード |
| --- | --- |
| invalid_arguments | 2 |
| not_found / ambiguous | 3 |
| network_error / http_error | 4 |
| invalid_response / parse_error | 5 |
| upstream_error | 6 |
| cache_error | 1 |

終了コード0は成功（有効な0件を含む）、1はその他、2は引数、3は未発見・曖昧、4は通信・HTTP、5は形式・解析、6はAPI内部エラーです。部分失敗で複数エラーがある場合は数値最大の終了コードを返します。

`--raw` は取得URLをキーとし、元のAPI本文を文字列として返します。JSON・XMLともBOM・空白・改行を保持します。外側のJSONでは文字列としてエスケープされます。失敗時も取得済み本文は出します。`--json` とは同時指定できません。docsにはAPI本文がないため `--raw` は引数エラーです。

`docs list --json` のdataは名前・概要（name・summary）の配列、`docs show --json` はname・summary・Markdown本文contentです。文書名は完全一致で指定します。文書はバイナリに埋め込まれ、通信・永続キャッシュを使いません。`--help` / `--version` は `--json` 指定時も通常の文字表示です。

静的マスター・時刻表はOS標準のユーザーキャッシュへ保存します。HTTPのCache-Control・ETag・Last-Modifiedを使い、期限指定がない場合はマスター24時間、時刻表6時間です。`--refresh` は再検証、`--no-cache` は永続キャッシュの読み書きを無効にします。再取得が失敗しても古いデータへ戻りません。運行情報・接近情報は毎回取得します。

`--timeout SECONDS` は1秒以上で既定値10秒。接続失敗・タイムアウト・HTTP 502/503/504のみ最大2回再試行します。内部APIに公開APIとしての互換性保証はありません。
