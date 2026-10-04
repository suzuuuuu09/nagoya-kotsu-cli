# 正規化済みJSONを共通形式で包む

`--json` の結果は、全コマンドで `schema_version`、`complete`、`data`、`errors` を持つオブジェクトにする。初期の `schema_version` は `1` とし、コマンド固有の正規化済みデータは `data` に置く。成功時は `complete: true` と `errors: []`、独立した取得の部分失敗時は `complete: false` と失敗情報を返す。部分失敗時の非0終了コードは [ADR-0004](0004-report-partial-results-as-errors.md) に従う。

コマンド固有の項目を最上位に置く形式は短くアクセスできる一方、全コマンドで結果と不完全さを共通に扱いにくい。利用側が同じ形式で判定できるように、共通形式を採用する。内部APIの生レスポンスを `data` に直接入れず、CLIの正規化済みモデルを公開する。

成功時の共通形式は次のとおり。

```json
{
  "schema_version": 1,
  "complete": true,
  "data": {},
  "errors": []
}
```

## コマンド全体の失敗と内蔵ドキュメント

`--json` 指定時は引数解析を含むコマンド全体の失敗も同じEnvelopeを使い、`complete: false`、`data: null` とする。失敗が起きた段階によって形式が変わると利用側が別の解析経路を必要とするため、失敗時も共通形式を維持する。`errors` の各項目には安定した機械判定用の `code` を追加し、既存の終了コードと部分失敗のデータ保持は変更しない。分類は [CLI動作仕様](../cli-behavior.md#jsonエラー) に従う。

内蔵ドキュメントも `--json` では同じEnvelopeを使う。文書本文はAPIの生レスポンスではないため、ドキュメントコマンドの `--raw` は引数エラーにする。
