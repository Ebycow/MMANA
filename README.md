# MMANA — アンテナ設計・解析プログラム

MMANA（Mininec-based Antenna ANAlyzer）は、**JE3HHT 森 誠OM**が開発されたWindows用アンテナ設計・解析ソフトウェアです。  
MININEC3の計算エンジンをベースに、ダイポールから複雑な多素子アレーまで、グラフィカルに設計・シミュレーションできます。

---

jl3oxr OM版をベースに、以下の変更を加えています。

### 新機能

- **クワッドビューモード（Quad View）**: アンテナの4方向同時表示ビューを追加。ワイヤ配置の把握が容易になります。
- **ミリメートル長表示の設定化**: 素子長などをmm単位で表示するかどうかを設定から変更できるようにしました。

### 改善

- **周波数ビューの更新処理改善**: 周波数設定変更時の描画更新を改善。
- **クラッシュ診断情報の強化**: 異常終了時のミニダンプ出力を改善し、問題の特定を容易にしました。

### バグ修正

- **TextEdit フォントピッチ列挙型の参照修正**: 新バージョンのC++ Builderで警告・エラーとなる列挙型参照を修正しました。
- **Main.cpp の DOS EOF マーカー除去**: ファイル末尾の不要な制御文字（`^Z`）を除去しました。

---

## ビルド方法

### 必要環境

**RAD Studio 12.1 Athens Community Edition**（無償）でビルドできることを確認しています。  
Community Editionは個人開発者・学生・スタートアップが無償で利用できます。

- ダウンロード: https://www.embarcadero.com/jp/products/cbuilder/starter

### 手順

1. `Mmana.cbproj` をRAD StudioまたはC++ Builder IDEで開く
2. ターゲットプラットフォーム（Win32 / Win64）を選択
3. ビルドを実行

> **注意**: Win32版とWin64版では最適化・計算結果の保存ファイル形式が異なります（バイナリ非互換）。

---

## ライセンス

GNU Lesser General Public License (LGPL) v3.0  
詳細: http://www.gnu.org/licenses/lgpl-3.0.en.html

Copyright 1999-2013 Makoto Mori (JE3HHT), Nobuyuki Oba (JA7UDE)

---

## 関連リポジトリ

| リポジトリ | 説明 |
|---|---|
| [`ja7ude/MMANA`](https://github.com/ja7ude/MMANA) | JA7UDE 大庭OM — XE4移植・オープンソース化 |
| [`7M4KSC/MMANA`](https://github.com/7M4KSC/MMANA) | 7M4KSC OM — 10.4対応・Win64対応 |
| [`jl3oxr/MMANA`](https://github.com/jl3oxr/MMANA) | jl3oxr OM — 多数の改善・11.3/12.1対応 |
| [`Ebycow/MMANA`](https://github.com/Ebycow/MMANA) | このリポジトリ — クワッドビュー等の追加 |
