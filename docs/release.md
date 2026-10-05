# リリース手順

GitHub Actionsの `.github/workflows/release.yml` は、`v0.1.0` のようなバージョンタグのpushで起動します。Cargo package名は `nagoya-kotsu-cli`、バイナリ名は `nkotsu` です。バージョン番号は手動で更新します。

## 配布物と検証

crates.ioにはソースパッケージを公開し、GitHub Releaseには自動生成したリリースノートと次のバイナリを添付します。

| OS・CPU | Rust target | runner | アーカイブ |
| --- | --- | --- | --- |
| Linux x86_64 | `x86_64-unknown-linux-gnu` | `ubuntu-24.04` | `.tar.gz` |
| Linux ARM64 | `aarch64-unknown-linux-gnu` | `ubuntu-24.04-arm` | `.tar.gz` |
| macOS Intel | `x86_64-apple-darwin` | `macos-15-intel` | `.tar.gz` |
| macOS Apple Silicon | `aarch64-apple-darwin` | `macos-15` | `.tar.gz` |
| Windows x86_64 | `x86_64-pc-windows-msvc` | `windows-2022` | `.zip` |

アーカイブ名は `nkotsu-v0.1.0-aarch64-apple-darwin.tar.gz` の形式です。各アーカイブにはバイナリ、`README.md`、`LICENSE`、`NOTICE`、`THIRD-PARTY-LICENSES.json` を含め、Releaseには `SHA256SUMS` も添付します。Linux版はUbuntu 24.04上でビルドするGNU版です。musl版、バイナリの署名、macOSの公証は行いません。

公開前に、タグとCargoのバージョン一致、リポジトリのpublic設定、フォーマット、Clippy、Rustとライセンス収集処理のテスト、Cargoパッケージの生成・ビルドを検証します。続いて5種類のバイナリをビルドし、各runnerでバージョンと内蔵ドキュメントを確認します。いずれかが失敗した場合は公開処理へ進みません。crates.ioへの公開が成功してからGitHub Releaseを作成します。

`v0.1.0-beta.1` のようにハイフンを含むバージョンタグは、GitHub Releaseをprereleaseとして作成します。通常のブランチpushやpull requestでは公開しません。

## 初回公開の準備

GitHubリポジトリはpublicで運用する方針です。公開対象とGit履歴を確認してから、リポジトリ設定でpublicへ変更してください。privateのままではrelease workflowの検証が失敗します。

[crates.ioのAPIトークン設定](https://crates.io/settings/tokens)で初回公開用のトークンを作成し、GitHubのrepository secretへ `CARGO_REGISTRY_TOKEN` という名前で登録してください。トークンは、公開stepの環境変数として渡します。ファイル、コマンド引数、ログへ値を書き込む必要はありません。GitHub Releaseの作成にはActionsが発行する `GITHUB_TOKEN` を使います。

新規crateの初回公開にはAPIトークンが必要です。初回公開を終えたら、次の節のTrusted Publishingへ切り替えます（[crates.io公式ドキュメント](https://crates.io/docs/trusted-publishing)）。

## Trusted Publishingへの切り替え

初回公開後、crates.ioのcrate設定でTrusted Publishingを追加します。

| 設定項目 | 値 |
| --- | --- |
| Repository owner | `suzuuuuu09` |
| Repository name | `nagoya-kotsu-cli` |
| Workflow filename | `release.yml` |
| Environment | 指定しない |

設定後はGitHubのsecret `CARGO_REGISTRY_TOKEN` を削除し、初回用APIトークンもcrates.ioで失効させます。secretが存在しない場合、workflowは公式の `rust-lang/crates-io-auth-action` でOIDC認証を行います。secretが残っている場合はAPIトークンを優先するため、切り替え時に削除してください。

## バージョン更新とタグ

`Cargo.toml` の `package.version` を更新し、`cargo check` で `Cargo.lock` にも反映します。初回の `0.1.0` を公開する場合は、バージョンを変更する必要はありません。変更内容を確認してコミットし、公開するコミットへタグを付けます。

```sh
cargo check
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
python3 -m unittest discover -s tests -p release.py
cargo package --locked
```

以下は `Cargo.toml` が `0.1.0` の場合の公開操作です。タグのpushでcrates.ioとGitHub Releaseへの公開が始まるため、初回認証の設定や変更内容の確認を先に済ませてください。

```sh
git tag v0.1.0
git push origin v0.1.0
```

## 途中で失敗した場合

crates.ioへ公開する前に失敗した場合は、原因を解消してActionsの失敗したjobを再実行できます。タグとバージョンの不一致など、ソースの修正が必要な場合は、新しいコミットと適切なバージョンタグを用意してください。

crates.ioへの公開後にGitHub Releaseの作成が失敗した場合は、GitHub Releaseのjobだけを再実行します。公開済みのcrateは同じバージョンで再公開できません。publish jobの成功を確認してから再実行してください（[Cargoの公開仕様](https://doc.rust-lang.org/cargo/reference/publishing.html)）。GitHub Releaseが既に作成されている場合は、添付済みのアーカイブを確認し、不足分だけを追加してください。

## ライセンスと同梱データ

プロジェクトのコードにはApache License 2.0（`Apache-2.0`）を適用します。`LICENSE` は公式のライセンス本文、`NOTICE` は著作権表示と祝日データの出典・加工内容・利用条件です。Cargoの公開パッケージにも両方を含めます。

依存ライブラリのライセンスは、`scripts/bundle_licenses.py` でtargetごとのCargo依存関係から収集します。サブディレクトリのライセンスとNOTICEも含め、ライセンスファイルが見つからない依存があればビルドjobを失敗させます。新しい依存関係を追加する際は、収集結果と利用条件も確認してください。

workflowとライセンスの準備だけでは公開は完了しません。public設定、初回認証、タグのpushを実行して初めて配布が始まります。
