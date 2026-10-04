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
