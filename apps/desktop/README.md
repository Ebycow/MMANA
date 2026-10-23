# MMANA デスクトップアプリ

Tauri 2 + React 19 + TypeScript + Mantine + three.js。

## 開発

```sh
npm install
npm run tauri dev        # デスクトップアプリとして起動（ホットリロード）
```

### ブラウザ版（GitHub Pages）

計算エンジンを WebAssembly（`crates/mmana-wasm`）にして、すべてをブラウザの中で動かします。
`wasm-bindgen` の CLI（`Cargo.lock` の wasm-bindgen と同じ版）と `wasm32-unknown-unknown` ターゲットが必要です。

```sh
cargo install wasm-bindgen-cli --version 0.2.129
rustup target add wasm32-unknown-unknown
npm run dev:web          # wasm をビルドして http://localhost:1420/?sample=3EL20.MAA
npm run build:web        # dist-web/ に静的ファイル一式（Pages では --base /<リポジトリ名>/ を付ける）
npm run test:wasm        # 全サンプルで wasm とネイティブ（target/release/mmana）の結果を比較
```

`npm run build:wasm` が `web/engine/` に wasm を書き出し、`--mode web` のときだけそこが配信されます。
公開は `.github/workflows/pages.yml` が `v*` タグで行い、リリースの間は手動実行で main を公開します。
どのコミットからビルドしたかは「ヘルプ > MMANAについて」に `git describe` の形（例: `v0.2.0-43-g060248a`）で出ます。

### ブラウザ + HTTP API

UI だけをブラウザで動かし、計算を HTTP API（`crates/mmana-api`）に送ることもできます。

```sh
cargo run --release -p mmana-api          # リポジトリのルートで
npm run dev                               # http://localhost:1420/?sample=3EL20.MAA
```

API の場所は `VITE_MMANA_API`（既定 `http://127.0.0.1:8080`）で変えられます。

## ビルド

```sh
npm run tauri build      # インストーラ（.dmg / .msi など）
npm run tauri build -- --no-bundle   # 実行ファイルのみ
```

実行ファイルに `.maa` を渡すと、そのファイルを開いて計算します（`mmana-desktop antenna.maa`）。

## 型の生成

TypeScript の型（`src/api/schema.d.ts`）は Rust の OpenAPI 定義から生成しています。
`mmana-core` / `mmana-api` の型を変えたら再生成してください。

```sh
npm run gen:api
```

## 構成

| パス | 内容 |
|---|---|
| `src-tauri/` | Tauri（Rust）。`mmana-core` を直接呼ぶコマンドとファイル入出力 |
| `src/api/` | 生成した型と、Tauri / wasm / HTTP を切り替える `Transport`、wasm を動かす Web Worker |
| `src/state/store.ts` | アプリの状態（zustand）。元に戻す・やり直す、計算の実行 |
| `src/domain/` | SEG 列の変換、数式入力、3D 用のジオメトリ、3D 編集の変形（`wireEdit.ts`）など（単体テストあり） |
| `src/components/` | 画面部品（ワイヤー表、3D ビュー、結果パネルなど） |
| `src/charts/` | 指向性図・周波数特性グラフ（SVG） |

## ショートカット

既定のキーとマウス操作です。**設定 → キーボードとマウス…**（⌘/Ctrl + ,）で、各操作のキーの追加・削除、
3D ビューでのマウスボタン（回転・平行移動・ズーム）、ズームの向きと速さ、押しながら操作するキー（範囲選択・選択に追加など）を変えられます。
設定はブラウザ（WebView）に保存され、ドキュメントには含まれません。

| キー | 操作 |
|---|---|
| ⌘/Ctrl + Enter | 計算 |
| ⌘/Ctrl + , | キーボードとマウスの設定 |
| ⌘/Ctrl + O / S / ⇧S | 開く / 保存 / 名前を付けて保存 |
| ⌘/Ctrl + Z / ⇧Z | 元に戻す / やり直す |
| Enter（表の中） | 確定して下の行へ |
| Esc | 編集の取り消し・描画の終了・ワイヤー選択の解除 |

### 3D ビューでの編集

| キー・操作 | 内容 |
|---|---|
| V / G / R | 選択 / 移動ギズモ / 回転ギズモ |
| W / Q / O | ワイヤーを描く / 四角ループ / 多角形ループ |
| クリック・Shift / ⌘ + クリック | 選択・選択に追加 / 外す（表の No. 欄は Shift で範囲） |
| Shift + ドラッグ（空いた所） | 範囲選択 |
| ダブルクリック | つながったワイヤー（エレメント・ループ）を選択 |
| ワイヤー・端点をドラッグ | 移動（ふだんは XY 面、視線が水平に近いときは正面の面）。Shift で 1 軸に固定、Alt で「つながりを保つ」を一時的に切り替え |
| 描画中に数字 + Enter | 長さ（多角形は半径）を指定 |
| Delete / Backspace | 選択したワイヤーを削除（給電点・負荷の番号は追従し、削除したワイヤー上のものは消えます） |
| ⌘/Ctrl + D / C / X / V / A | 複製 / コピー / 切り取り / 貼り付け / すべて選択 |

スナップ（グリッド・端点と中点・ワイヤー上・15° 刻み）、作図面、つながりを保つかどうか、途中に付けたら分けてつなぐかどうかは、3D ビュー左上の磁石ボタンで切り替えます。

計算では、ワイヤーは端点の座標が完全に一致したときだけつながります。描いたり動かしたりした端がほかのワイヤーの途中に乗ると、そのワイヤーをそこで分けてつなぎます（給電点・負荷は同じ位置の名前に付け替えます。中央の `w1c` を中央で分けると `w1e`）。
磁石ボタンの隣の接続チェックは、見た目は触れているのに計算上はつながっていない箇所（途中に触れた端、半径の和より近いのに一致しない端点、重なったワイヤー）を赤い数字で知らせ、1 か所ずつ、またはまとめて直せます。
途中で交わるワイヤーも一覧に出しますが、たすき掛けの位相線（HB9CV・LPDA など）はつながないのが正しいため、まとめて直す対象には含めません。
