---
name: nagoya-kotsu
description: nkotsu CLIで名古屋市交通局の市バス・地下鉄の運行情報、停留所、時刻表、接近情報、経路、普通運賃、定期券料金、延着証明書、駅・バス停の検索と位置関係を調べる。名古屋市営交通について利用者から問い合わせを受けたときに使う。
---

# 名古屋市営交通を調べる

`nkotsu` で情報を取得し、利用者の条件に合う便や経路を案内する。CLIの開発・リリース作業は対象外。

## 実行前の確認

`nkotsu --version` で実行できることを確認する。見つからない場合は、`cargo install nagoya-kotsu-cli --locked` またはGitHub Releasesのバイナリで導入できることを伝える。インストールまで依頼されていなければ、導入案内で止める。

インストールされた版の仕様は `nkotsu --help` と内蔵docsで確認する。ソースのチェックアウトやWeb上の説明を前提にしない。

## 問い合わせからコマンドを選ぶ

| 調べる内容 | コマンド | 必要なときに読む文書 |
| --- | --- | --- |
| 運行状況・運行変更の記事 | `nkotsu status` | `nkotsu status --help` |
| バス停とのりば | `nkotsu bus stop <停留所>` | `nkotsu docs show bus` |
| バスの予定便 | `nkotsu bus timetable <停留所>` | `nkotsu docs show bus` |
| バスの現在位置・通過履歴 | `nkotsu bus live <停留所>` | `nkotsu docs show bus` |
| 地下鉄の時刻表 | `nkotsu subway timetable <駅>` | `nkotsu docs show subway` |
| 地下鉄の次の予定列車 | `nkotsu subway next <駅>` | `nkotsu docs show subway` |
| 出発地から到着地までの経路 | `nkotsu route <出発地> <到着地>` | `nkotsu docs show route` |
| 普通運賃 | `nkotsu fare <FROM> <TO>` | `nkotsu docs show fare` |
| 定期券料金 | `nkotsu pass <FROM> <TO>` | `nkotsu docs show pass` |
| 延着証明書 | `nkotsu delay-cert` | `nkotsu docs show delay-cert` |
| 地点を探す | `nkotsu search <QUERY>` | `nkotsu docs show places` |
| 地下鉄駅の基本情報 | `nkotsu station <STATION>` | `nkotsu docs show places` |
| 駅・バス停の座標 | `nkotsu location <PLACE>` | `nkotsu docs show places` |
| 周辺の交通施設 | `nkotsu nearby <PLACE>` | `nkotsu docs show places` |

該当する文書とサブコマンドの `--help` を読み、必要なオプションを選ぶ。出力の読み方は `nkotsu docs show output`、失敗への対処は `nkotsu docs show troubleshooting` を参照する。

### 場所と検索条件

利用者が指定した交通手段・系統・のりば・方面を条件に反映する。バスののりばが必要なら `bus stop` で確認する。

地点名に複数候補がある場合は、指定済みの交通手段や場所で候補を絞る。条件から決められなければ、返された候補を示して利用者に選んでもらう。同名の駅とバス停を勝手に選ばない。経路検索は `--bus` または `--subway` で絞り込める。両方指定すると両方が対象になる。

曖昧な地点は `nkotsu search 藤が丘 --json` で候補を探す。選んだ `qualified_name`（例: `藤が丘(名古屋市地下鉄)`）を `route`・`station`・`location`・`nearby` へ渡す。既存の `bus`・`subway`・`fare`・`pass` には `name` を渡す。内部IDを受け渡しに使わない。`search` の路線情報は所属路線を網羅せず、のりばは `bus stop` や時刻表で確認する。

`nearby` の距離は概算直線距離として伝え、徒歩距離・徒歩時間に置き換えない。基準地点の種別は `--origin-type`、結果の種別は `--type` で指定する。`location` と `nearby` は利用者や端末の現在位置を取得しない。

日時は日本時間で扱う。経路検索の `--at HH:MM` は今日の日付になり、過去の時刻でも翌日には繰り越さない。別の日を調べるときは `YYYY-MM-DDTHH:MM` を使う。出発時刻と到着期限を区別し、到着期限なら `--arrive` を指定する。

時刻表の営業日は04:00が境界で、深夜00〜03時台は前の営業日の24〜27時台に相当する。経路検索の暦日と混同せず、返された便の日付を確認する。日種は自動判定を基本とし、`--day` で上書きする場合は対象日の適用を確認する。

## 取得結果を読む

交通情報を取得するコマンドには `--json` を付け、stdoutのJSONと終了コード、stderrを確認する。

- `schema_version` を確認し、内蔵docsでその版の形式を読む。結果は `data`、完全性は `complete`、失敗は `errors` にある。
- `complete: false` または非0の終了コードでは、取得できた内容と失敗した範囲を分けて伝える。一部の成功を全体の成功として扱わない。
- 終了コード0で結果が0件の場合は、指定条件に該当する結果がないと伝える。通信失敗や、運行全体の停止とは区別する。
- エラー種別は `errors[].code` で確認する。引数エラーはhelp、曖昧性は候補選択、通信・解析エラーはtroubleshootingに従って対処する。

`--raw` は解析トラブルの調査に必要な場合に使う。`--json` と同時指定できず、内蔵docsでも使えない。CLIには通信の自動再試行があるため、失敗を繰り返し再実行するより、原因と取得できなかった範囲を報告する。

## 案内する内容

利用者が必要とする発着時刻、系統・路線、方面、のりば、乗換、運賃などを、取得できた項目から示す。検索条件と日付、取得日時が返されていればその日時も添える。運行記事の作成日時と情報の取得日時は区別する。

地下鉄の次発は時刻表に基づく予定列車として案内する。市バスの接近情報は取得した現在位置と通過履歴として案内し、履歴だけなら「現在位置情報なし」と明記する。履歴から現在位置、到着予測、遅延分、GPS座標を推定しない。

料金検索は利用可能な料金経路をすべて返す。掲載順や運賃だけでは最短・推奨とする根拠にならないため、先頭や最安をそのように解釈しない。

定期券料金には利用者の資格情報が含まれないため、料金から購入資格を判定しない。券種候補が不完全な場合の指定は `nkotsu docs show pass` で確認する。

延着証明書は過去の遅延を証明するものであり、現在の遅延情報ではない。証明書が古い・0件であることから現在の平常運行や遅延時間を推測せず、現在の運行状況は `status` で確認する。

## 実行例

```sh
nkotsu search 藤が丘 --json
nkotsu location '藤が丘(名古屋市地下鉄)' --json
nkotsu nearby '藤が丘(名古屋市地下鉄)' --type bus --json
nkotsu status --line 東山線 --json
nkotsu bus stop 上社 --json
nkotsu bus live 上社 --route 上社12 --json
nkotsu subway next 栄 --line 東山線 --direction 藤が丘方面 --limit 3 --json
nkotsu route 藤が丘 名古屋 --subway --at 2026-10-06T09:00 --arrive --json
```

例の地点・日時・条件は、利用者の指定に置き換える。
