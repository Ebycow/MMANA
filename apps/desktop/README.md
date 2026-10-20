# MMANA デスクトップアプリ

Tauri 2 + React 19 + TypeScript + Mantine + three.js。

## 開発

```sh
npm install
npm run tauri dev        # デスクトップアプリとして起動（ホットリロード）
```

UI だけをブラウザで動かすこともできます。計算は HTTP API（`crates/mmana-api`）に送られます。

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
| `src/api/` | 生成した型と、Tauri / HTTP を切り替える `Transport` |
| `src/state/store.ts` | アプリの状態（zustand）。元に戻す・やり直す、計算の実行 |
| `src/domain/` | SEG 列の変換、数式入力、3D 用のジオメトリなど（単体テストあり） |
| `src/components/` | 画面部品（ワイヤー表、3D ビュー、結果パネルなど） |
| `src/charts/` | 指向性図・周波数特性グラフ（SVG） |

## ショートカット

| キー | 操作 |
|---|---|
| ⌘/Ctrl + Enter | 計算 |
| ⌘/Ctrl + O / S / ⇧S | 開く / 保存 / 名前を付けて保存 |
| ⌘/Ctrl + Z / ⇧Z | 元に戻す / やり直す |
| Enter（表の中） | 確定して下の行へ |
| Esc | 編集の取り消し・ワイヤー選択の解除 |
