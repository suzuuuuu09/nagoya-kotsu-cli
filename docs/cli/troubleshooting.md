# トラブルシューティング

```sh
nkotsu docs list
nkotsu bus stop 上社 --no-cache --verbose
nkotsu route 藤が丘 名古屋 --subway --json
```

- `invalid_arguments`: サブコマンドの `--help` で必須引数・指定値・競合を確認してください。`--json --raw`、`--verbose --quiet` は同時指定できません。docsでは `--raw` を使用できません。
- `not_found`: 入力名や系統・のりば・路線・方面を確認し、提示された候補を指定してください。docsの名前はbus、subway、route、output、troubleshootingです。
- `ambiguous`: 候補をより具体的に指定してください。経路検索の「藤が丘」は `--subway` で地下鉄駅に絞れます。CLIは曖昧な候補を勝手に選びません。
- `network_error`: 接続状況を確認してください。必要に応じて `--timeout` を増やし、`--verbose` で詳細を確認します。
- `http_error`: 上流のHTTPエラーです。一時的な障害の場合は時間を置いて再実行してください。502/503/504は自動再試行します。
- `invalid_response` / `parse_error`: HTMLなど想定外の本文、形式の変更や破損の可能性があります。`--verbose` で診断を確認し、`--raw` で取得済みの本文を確認できます。`--json` と `--raw` は別の実行で指定してください。
- `upstream_error`: API内部のエラーです。表示されたメッセージを確認してください。
- `cache_error`: `--no-cache` で永続キャッシュを無効にして再実行できます。

自動の日種判定ができない場合は `--day` を明示してください。市バスはweekday・saturday・holiday、地下鉄はweekday・holiday・new-year・all-nightです。特別ダイヤが実際に適用されるかを確認してください。

0件の成功と取得失敗は別です。部分失敗時は成功データが残りますが、`complete: false` と非0終了コードを必ず確認してください。再取得が失敗しても古いキャッシュへ戻りません。

接近情報は到着予測ではなく、地下鉄の次発は時刻表に基づく予定列車です。現在位置がなければ通過履歴から位置や遅延を推定しません。
