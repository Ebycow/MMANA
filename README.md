# MMMANA — MMANA の Rust 移植

JE3HHT 森 誠氏の MMANA（MININEC3 ベースのアンテナ解析ソフト）の計算エンジンを Rust に移植し、
デスクトップアプリ・ブラウザ・HTTP API・CLI から使えるようにしたものです。

**移植した計算エンジンは、オリジナルの C++ エンジンとビット単位で同じ結果を返します**
（サンプル 81 ファイルとその派生、計 149 ケースで検証済み。詳細は [docs/architecture.md](docs/architecture.md)）。
エレメント編集も、オリジナルの `CWGroup` / `CWGroupList` とビット単位で同じワイヤを書き出します（81 ファイル × 8 種の編集手順、計 648 ケース）。
最適化も、オリジナルの `CACal` と同じ値を同じ順に試し、同じアンテナで終わります（25 ケースで、計算ごとの値・評価値・試行の記録・最終形状を検証）。

画面・ファイル入出力を含む [移植状況と残作業](docs/architecture.md#移植状況と残作業) と、
互換性の例外をまとめた [移植メモ](docs/porting-notes.md) も参照してください。

## 構成

| ディレクトリ | 内容 |
|---|---|
| [crates/mmana-core](crates/mmana-core) | 計算エンジン本体。純 Rust、I/O なし（WebAssembly でもビルド可能） |
| [crates/mmana-api](crates/mmana-api) | HTTP API（axum、OpenAPI 3 対応） |
| [crates/mmana-cli](crates/mmana-cli) | コマンドラインツール `mmana` |
| [crates/mmana-wasm](crates/mmana-wasm) | ブラウザ版の計算エンジン（WebAssembly） |
| [apps/desktop](apps/desktop) | デスクトップアプリ（Tauri 2 + React/TypeScript） |
| [reference/oracle](reference/oracle) | オリジナルの `mininec3.cpp` を無改変でビルドし、正解データを作るハーネス |
| [testdata](testdata) | サンプル `.MAA` と正解データ（golden） |
| [docs](docs) | 設計資料・移植メモ |

## 使い方

Rust 1.88 以上と Node.js 22 以上が必要です（Rust は [rustup](https://rustup.rs) でインストール）。

### デスクトップアプリ

```sh
cd apps/desktop
npm install
npm run tauri dev
```

詳しくは [apps/desktop/README.md](apps/desktop/README.md) を参照してください。

### ブラウザ版

同じ画面を、計算エンジンごとブラウザで動かせます（GitHub Pages で公開。リリースのタグを打つと更新されます）。
計算は WebAssembly でブラウザの中だけで行い、結果はデスクトップ版と同じです。並列計算をしないので、大きなアンテナや最適化はデスクトップ版より遅くなります。

```sh
cargo install wasm-bindgen-cli --version 0.2.129   # Cargo.lock の wasm-bindgen と同じ版
rustup target add wasm32-unknown-unknown
cd apps/desktop
npm run dev:web          # http://localhost:1420/
```

### Rust（エンジン・API・CLI）

```sh
# テスト（オリジナルとの一致検証を含む。デスクトップ以外のクレート）
cargo test --release

# CLI
cargo run --release -p mmana-cli -- calc testdata/maa/3EL20.MAA
cargo run --release -p mmana-cli -- sweep testdata/maa/3EL20.MAA --start 13.9 --stop 14.4 --points 6
cargo run --release -p mmana-cli -- optimize testdata/maa/3EL20.MAA --setup setup.json -o out.maa

# API サーバー（http://127.0.0.1:8080）
cargo run --release -p mmana-api
```

### API

| メソッド | パス | 内容 |
|---|---|---|
| GET | `/health` | 稼働確認 |
| GET | `/openapi.json` | OpenAPI ドキュメント |
| POST | `/v1/maa/parse` | `.maa` ファイル（Shift_JIS / UTF-8）→ プロジェクト JSON |
| POST | `/v1/maa/export` | プロジェクト JSON → `.maa`（`?encoding=shift_jis` 既定 / `utf-8`） |
| POST | `/v1/calculations` | インピーダンス・SWR・利得・F/B・パターン・電流分布 |
| POST | `/v1/sweeps` | 周波数特性 |
| POST | `/v1/far-field` | 任意方向の遠方界 |
| POST | `/v1/near-field` | 近傍界（電界・磁界） |
| POST | `/v1/elements` | エレメント編集（つながったワイヤをエレメントとして、幅・全周・間隔などを変更） |
| POST | `/v1/optimizations` | 最適化（寸法・集中定数・地上高などを変えて、利得・F/B・SWR などの目標に近づける） |
| POST | `/v1/optimizations/setup` | 最適化の変数表の編集（全エレメント・自動連携登録など）と、現在値・問題点の確認 |
| POST | `/v1/optimizations/apply` | 最適化の試行結果をアンテナに書き戻す |

エラーは [RFC 9457](https://www.rfc-editor.org/rfc/rfc9457) の `application/problem+json` で返します。

```sh
curl -s --data-binary @testdata/maa/3EL20.MAA localhost:8080/v1/maa/parse \
  | jq '{project}' \
  | curl -s -H 'content-type: application/json' -d @- localhost:8080/v1/calculations \
  | jq '{impedance, swr, gainDbi, fbDb, elevationDeg}'
```

環境変数: `MMANA_ADDR`（待ち受けアドレス）、`MMANA_MAX_PULSES`（パルス数の上限）、`MMANA_MAX_OPTIMIZE_CALCULATIONS`（最適化 1 回の計算回数の上限、既定 2000）、`RUST_LOG`。

## ライセンス

GNU Lesser General Public License version 3 以降（[LICENSE](LICENSE)、LGPL が参照する GPL 本文は [COPYING](COPYING)）。オリジナルの MMANA を受け継いでいます。

Copyright 1999-2013 Makoto Mori (JE3HHT), Nobuyuki Oba (JA7UDE)
