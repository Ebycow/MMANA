# アーキテクチャ

## 全体像

```
                ┌──────────────── mmana-core（純 Rust・I/O なし）────────────────┐
                │  model     公開データモデル（Project / Antenna / Environment）  │
                │  maa       .maa の読み書き（Shift_JIS / UTF-8）                 │
                │  calc      calculate / sweep / far_field / near_field           │
                │  pattern   パターン走査と利得・F/B・打ち上げ角（CRes 相当）      │
                │  elements  エレメント編集（CWGroup / CWGroupList の移植）        │
                │  optimize  最適化（CACal の移植）                               │
                │  engine    MININEC3 本体（mininec3.cpp の移植）                  │
                └──────▲───────────────────▲──────────────────────▲───────────────┘
                       │                   │                      │
              mmana-api（axum）      mmana-cli（clap）     apps/desktop（Tauri 2）
              REST + OpenAPI          バッチ・検証用        Tauri 2 のコマンドから直接呼ぶ
                       ▲                                          ▲
                       └──────── React/TypeScript の UI ──────────┘
                                 （同じ型を使い、HTTP / Tauri IPC を切り替え）
```

- **計算は `mmana-core` に閉じ込めます。** ファイルやネットワークに触れないので、
  API サーバー、CLI、Tauri、WebAssembly（ブラウザ版）のどこからでも同じコードが動きます。
  内部の並列計算（[性能](#性能)）は rayon で行い、WebAssembly では逐次実行になります。
- **API は薄い層です。** JSON の入出力、制限（パルス数・点数・ボディサイズ）、エラー整形だけを担い、
  計算は `spawn_blocking` で別スレッドに逃がします。
- **UI との契約は `mmana_core::contract` です。** リクエストとレスポンスの型を core に置き、
  HTTP API と Tauri コマンドの両方がそれを受け渡します。OpenAPI 文書から TypeScript の型を生成するので
  （`npm run gen:api`）、UI は `Transport` を差し替えるだけで、デスクトップ（Tauri IPC）でもブラウザ（HTTP）でも動きます。

## デスクトップアプリ（apps/desktop）

| 層 | 技術 | 役割 |
|---|---|---|
| `src-tauri` | Tauri 2 | `contract` の関数を `spawn_blocking` で呼ぶコマンド、ファイルの読み書き、起動引数のファイル |
| `src/api` | 生成型 + `Transport` | Tauri / HTTP の切り替え、エラーを problem+json の形にそろえる |
| `src/state` | zustand | プロジェクト、元に戻す・やり直す（最大 200 手）、計算結果と「古い」状態、自動計算 |
| `src/components` | React 19 + Mantine 9 | ワイヤー表（数式入力・Enter で下へ）、エレメント編集ダイアログ、各種フォーム、結果パネル |
| `src/components/AntennaView` | three.js（react-three-fiber） | 3D 表示（Z 軸上向き）、ワイヤー選択、電流分布の色分け、視点プリセット。別チャンクで遅延読み込み |
| `src/charts` | SVG | 指向性図（MMANA と同じ向きと 10^(dB/40) スケール）、周波数特性（十字線を全グラフで共有） |

グラフの色は、色覚の多様性を考慮して検証済みのパレットから選び、ライト・ダークそれぞれの値を `styles.css` に定義しています。

## 計算エンジンの移植方針

オリジナルの `mininec3.cpp` は、BASIC の MININEC3 を行単位で C に写したもので、
約 200 個のグローバル変数と `goto` で動いています。移植では次の 2 点を両立させました。

1. **数値は 1 ビットも変えない。** 演算の順序、`float`（f32）で持つインピーダンス行列、
   `float` と `double` が混ざる式の精度まで、そのまま再現しています。
   たとえば `ZR*ZR + ZI*ZI` は f32 で計算してから f64 に変換し、`POW()` は `powi` ではなく掛け算の繰り返しで計算します。
2. **構造はモダンにする。** グローバル状態は `Mininec` 構造体に集め、各ルーチンが実際に使う値は引数で受け渡します。
   `goto` はループとラベル付きブロックに置き換えました。BASIC のサブルーチン名（S196、L135 など）は
   コメントに残してあるので、原典と並べて読めます。

| 原典 | 移植先 |
|---|---|
| `InitAntData`, `S1299` | `engine/geometry.rs` |
| `SetWZ`, `Tei*`, `CombWire` | `engine/segment.rs` |
| `Str2PlusNo`, `AdjPlusNo` | `engine/pulse.rs` |
| `L135`, `S28`, `S56/66/75/87/102`, `L113` | `engine/kernel.rs` |
| `S196`（行列生成・負荷・LU 分解・求解） | `engine/matrix.rs` |
| `S497`, `S572`, `S607`（電流分布） | `engine/currents.rs` |
| `S621`, `S184`（遠方界・実大地） | `engine/farfield.rs` |
| `S875`（近傍界） | `engine/nearfield.rs` |
| `CalCurrent`, `CRes`（パターン・利得・F/B） | `pattern.rs` |
| `LoadAntStrings` / `SaveAntStrings` | `maa.rs` |
| `Calc`（数式入力） | `expr.rs` |
| `CWGroup`, `CWGroupList`, `TGrpWireDlg` の編集規則 | `elements.rs` |
| `CACal`, `CACal::Exec`, `ACalExecute`, `TACalDlg` の変数操作 | `optimize.rs` |

## 検証の仕組み（Oracle テスト）

```
MMANA の C++ ソース ──extract.sh──▶ reference/oracle/generated/（行範囲で無改変コピー）
                                          │ clang -O2 -ffp-contract=off
                                          ▼
testdata/maa/*.MAA ──▶ oracle ──▶ testdata/golden/*.json（電流・インピーダンス・パターン…）
                                          │
                     crates/mmana-core/tests/golden.rs が Rust 版と比較
```

- `reference/oracle/extract.sh` が、オリジナルの `mininec3.cpp` と `ComLib.cpp` の必要な関数を
  **行範囲指定でそのままコピー**します。VCL への依存は `shim/` の最小限のスタブで置き換えています。
- `-ffp-contract=off` で、clang が `a*b+c` を FMA 命令にまとめるのを防ぎます（Rust は FMA に変換しないため）。
- 正解データは、サンプル 81 ファイル＋大地・高さ・周波数・近傍界・遠方界の派生ケース、計 149 件です。
- 正解データを作った macOS/arm64 では、**全ケースの全数値がビット単位で一致すること**をテストで要求しています。
  他の OS では libm の 1ulp の差が出うるため、表示桁よりずっと小さい許容誤差で比較します。

正解データの再生成:

```sh
reference/oracle/extract.sh ../MMANA     # オリジナルのソースから抽出（必要なときだけ）
reference/oracle/build.sh
reference/oracle/gen_golden.sh
```

オリジナルとの意図的な違いは [porting-notes.md](porting-notes.md) にまとめています。

## エレメント編集（elements）

端どうしがつながったワイヤを 1 つのエレメントとして扱い、形式（垂直□、水平DP など）を判定して、
横幅・縦幅・全周・半径・分割数・間隔・位置をまとめて変更する機能です（オリジナルの `GrpWire.cpp` のダイアログと、
`ComLib.cpp` の `CWGroup` / `CWGroupList`）。

- **状態を持たない API にしています。** オリジナルのダイアログはワイヤの写しを編集し、OK で書き戻します。
  `elements::edit_antenna`（API は `POST /v1/elements`）は、ダイアログを開いたときのアンテナと
  それまでの操作の列を受け取り、毎回最初から再生して、エレメント表と書き戻したアンテナを返します。
  UI はエレメントの状態を持たず、操作を戻すのは列の末尾を除くだけです。再生は 1 ms もかかりません。
- **ワイヤはオリジナルとビット単位で一致します。** エレメント編集の計算は四則演算と平方根だけで、
  IEEE 754 ではどの環境でも同じ値になるため、テストは全プラットフォームで完全一致を要求します。
- **検証は計算エンジンと同じ方式です。** `extract.sh` が `CWGroup` / `CWGroupList` を行範囲でそのまま抜き出し、
  `reference/oracle/elements_main.cpp`（ダイアログの編集規則を VCL なしで書き写したもの）が
  `testdata/elements/scripts/*.txt` の編集手順を全サンプルに適用して `testdata/elements/golden` を作ります
  （`reference/oracle/gen_elements.sh`）。`crates/mmana-core/tests/elements.rs` がそれと比較します。
- オリジナルの `AnaLoop` は、ループが閉じないときワイヤ配列の 1 つ先に書き込みます。
  Oracle では配列の確保に余白を足してこの書き込みを受け止めます（計算用の Oracle とは別のバイナリにしています）。

## 最適化（optimize）

ワイヤの座標・エレメントの寸法・集中定数・地上高・周波数・給電点・スタック間隔を変数にして、
利得・F/B・jX・SWR・打ち上げ角・マッチング回路・電流の重み付きの評価値が上がるように、変数を 1 つずつ動かす機能です
（オリジナルの `CACal`、`Main.cpp` の `CACal::Exec` と `ACalExecute`、設定ダイアログ `ACalBox.cpp` の変数操作）。

- **探索はオリジナルと同じ値を同じ順に試します。** 変数ごとに評価値が上がる間は歩幅を倍にして進み、下がったら戻るという手順、
  直近 128 件の試行の記録（同じ組み合わせは計算せずに記録の評価値を使う）、評価値の式（`RESL` が `float` で持つ値を `float` のまま使う箇所を含む）を
  文単位で写しています。エレメント変数は `elements::ElementList` を使い、オリジナルと同じくエレメントの境界を解析し直しません。
- **状態を持たない API です。** `optimize::optimize` はアンテナと設定（MMANA の「最適化シート」）を受け取り、最良の値を反映したアンテナと
  直近の試行を返します。試行の書き戻しは `apply_trial`、変数表の編集（全エレメント・自動連携登録・挿入・削除）は `edit_setup` が、
  エレメント編集と同じく操作の列を受け取って毎回最初から再生します。
- **途中経過と中止。** `optimize` は計算のたびにコールバックを呼び、`false` が返ると、その時点までの最良の状態で終わります。
  デスクトップアプリは Tauri のチャネルで途中経過を UI に送り（評価値が上がった計算は必ず、それ以外は 40 ms ごと）、中止ボタンでフラグを立てます。
  HTTP API は計算回数の上限（`MMANA_MAX_OPTIMIZE_CALCULATIONS`）までで打ち切ります。
- **検証は計算エンジンと同じ方式です。** `extract.sh` が `CACal`・`CACal::Exec`・`TACalDlg` の変数操作を行範囲でそのまま抜き出し、
  `reference/oracle/optimize_main.cpp` が `testdata/optimize/cases/*.txt` の設定で実行して、計算ごとの値・結果・評価値、試行の記録、
  最終的なアンテナを `testdata/optimize/golden` に書き出します（`reference/oracle/gen_optimize.sh`）。
  `crates/mmana-core/tests/optimize.rs` がそれと比較します。試す値は四則演算だけで決まるのでどの環境でも一致しますが、評価値は計算エンジンの結果なので、
  正解データを作った macOS/arm64 以外では許容誤差で比較し、評価値がわずかに異なって探索が分かれた時点で比較をやめます。

## 性能

数値を 1 ビットも変えずに速くするため、**各要素が受ける演算の列はそのままにして、計算の順序と共有だけを変えています。**
どの並列部分も独立な値を計算して元の順に並べるだけなので、結果はスレッド数によらず同じです（golden テストで検証）。

| 箇所 | 工夫 |
|---|---|
| 行列の作成（`engine/matrix.rs` の `fill`） | オリジナルは要素を順に埋めながら、対称性で k = +1 の値を後の行へコピーし、コピー先の計算を省きます。コピー先は形状だけで決まるので、① コピー先を洗い出す → ② 残りを行ごとに並列計算 → ③ コピーを元の順に再現 → ④ 大地のイメージ項を並列に加算、の 4 段で同じ行列を作ります。 |
| S87（スカラーポテンシャルの PSI） | 同じ引数で最大 4 回呼ばれる（隣の列と、ワイヤー上の隣の行）ので、行ブロックごとにキャッシュします（`PsiCache`）。引数だけで決まる関数なので値は同一です。 |
| ガウス消去（`factor`） | 32 列ずつのパネルに分け、パネルの外の列には各ステップの更新を元の順に（毎回 f32 に丸めて）まとめて適用し、行ごとに並列化します。元のループ順とのビット一致はランダム行列の単体テストでも確認しています。 |
| 遠方界（`far_field`） | 角度は元どおり累積加算で並べ、方向ごとに並列に計算します。パルスの両端で同じになる位相の sin/cos と、実大地の反射係数は 1 回だけ計算します。 |
| 周波数特性（`sweep`） | 周波数ごとに並列に計算します。 |

Apple M1（8 コア）での計測例。左の 2 列は 1 周波数の計算、右の 2 列はデスクトップアプリ既定の 21 点の周波数特性です（ms）。

| サンプル | パルス数 | 計算: 高速化前 | 計算: 現在 | 周波数特性: 高速化前 | 周波数特性: 現在 |
|---|---:|---:|---:|---:|---:|
| VERT20 | 8 | 0.47 | 0.29 | 8.1 | 1.6 |
| GP40 | 69 | 6.9 | 1.9 | 147 | 30 |
| 3HENT | 291 | 42 | 11 | 1,086 | 189 |
| 4X12 | 508 | 129 | 31 | 2,744 | 541 |
| CR2 | 856 | 460 | 112 | 9,796 | 1,815 |

1 スレッドでも、S87 のキャッシュと遠方界の重複計算の削除により、オリジナルの C++（clang -O2）より 2〜3 割速くなっています。

```sh
cargo run --release -p mmana-core --example bench   # calculate と 21 点の sweep（RAYON_NUM_THREADS=1 で 1 スレッド）
reference/oracle/bench.sh                            # オリジナルの C++ エンジンとの比較
```

## 次のフェーズ

1. 3D ビューでの直接編集（ワイヤー描画、移動・回転ギズモ、スナップ）
2. 最適化シートの保存・読み込み（設定と試行の一覧をファイルに残す）
3. 結果の比較、近傍界・遠方界ファイルの書き出し、印刷 / PDF、インストーラとファイル関連付け
