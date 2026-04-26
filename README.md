# MMANA — アンテナ設計・解析プログラム

MMANA（Mininec-based Antenna ANAlyzer）は、**JE3HHT 森 誠OM**が開発されたWindows用アンテナ設計・解析ソフトウェアです。  
MININEC3の計算エンジンをベースに、ダイポールから複雑な多素子アレーまで、グラフィカルに設計・シミュレーションできます。

---

jl3oxr OM版をベースに、以下の変更を加えています。

### 新機能

- **クワッドビューモード（Quad View）**: 従来四つのタブを切り替えて操作していたUIを同時表示できるようにしました。四タブ分をそのままのサイズで並べるため、HDサイズのディスプレイでは問題が発生する可能性があります。
- **ミリメートル長表示の設定化**: 素子長などをmm単位で表示するかどうかを設定から変更できるようにしました。

### 改善

- **周波数ビューの更新処理改善**: 周波数設定変更時の描画更新を改善。
- **クラッシュ診断情報の強化**: 異常終了時のミニダンプ出力を改善し、問題の特定を容易にしました。

### バグ修正

- **TextEdit フォントピッチ列挙型の参照修正**: 新バージョンのC++ Builderで警告・エラーとなる列挙型参照を修正しました。
- **Main.cpp の DOS EOF マーカー除去**: ファイル末尾の不要な制御文字（`^Z`）を除去しました。

---

## ビルド方法

**RAD Studio 12.1 Athens Community Edition**（無償）でビルドできることを確認しています。  
Community Editionは個人開発者・学生・スタートアップが無償で利用できます。

- ダウンロード: https://www.embarcadero.com/jp/products/cbuilder/starter

### コマンドラインビルド

RAD Studio / C++ Builder は MSBuild をビルドエンジンとして使用します。  
コマンドラインからビルドするには、まず `rsvars.bat` で環境変数を設定してから `msbuild` を呼び出します。

#### 1. 環境変数の設定

`rsvars.bat` は RAD Studio インストール先の `bin` フォルダにあります。  
RAD Studio 12.x Athens の場合、通常は以下のパスです。

```cmd
call "C:\Program Files (x86)\Embarcadero\Studio\23.0\bin\rsvars.bat"
```

インストール先や正確なバージョン番号は環境によって異なります。以下のコマンドで確認できます。

```cmd
dir "C:\Program Files (x86)\Embarcadero\Studio\"
dir "C:\Program Files\Embarcadero\Studio\"
```

表示された最新のバージョン番号フォルダ内の `bin\rsvars.bat` を `call` してください。

#### 2. ビルドコマンドの基本形

```cmd
msbuild Mmana.cbproj /p:Config=<構成> /p:Platform=<プラットフォーム>
```

**Config（ビルド構成）**

| 値 | 説明 |
|---|---|
| `Release` | 最適化あり・配布用 |
| `Debug` | デバッグ情報あり・開発用 |

**Platform（プラットフォーム）**

| 値 | 説明 |
|---|---|
| `Win32` | 32ビット版 |
| `Win64` | 64ビット版 |

#### 3. 実行例

```cmd
:: 環境変数の設定（毎回必要）
call "C:\Program Files (x86)\Embarcadero\Studio\23.0\bin\rsvars.bat"

:: Win32 Release ビルド
msbuild Mmana.cbproj /p:Config=Release /p:Platform=Win32

:: Win64 Release ビルド
msbuild Mmana.cbproj /p:Config=Release /p:Platform=Win64

:: Win32 Debug ビルド
msbuild Mmana.cbproj /p:Config=Debug /p:Platform=Win32

:: クリーンビルド（中間ファイルを削除してから再ビルド）
msbuild Mmana.cbproj /t:Clean;Build /p:Config=Release /p:Platform=Win32

:: 並列ビルド（コア数を指定して高速化）
msbuild Mmana.cbproj /p:Config=Release /p:Platform=Win32 /m:4
```

#### 4. 出力先

ビルド成功後、実行ファイルはプロジェクトルートに出力されます。

| 構成 | 中間ファイル格納先 | 実行ファイル |
|---|---|---|
| `Release` | `Release_Build\` | `Mmana.exe`（プロジェクトルート） |
| `Debug` | `Debug_Build\` | `Mmana.exe`（プロジェクトルート） |

> **注意**: Win32版とWin64版では最適化・計算結果の保存ファイル形式が異なります（バイナリ非互換）。同一環境でWin32とWin64を切り替える場合は、それぞれのファイルを別途保管してください。

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
