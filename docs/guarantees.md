# 保証一覧

計算エンジン（`mmana-core`）と HTTP API が**何を保証しているか**と、それを確かめているテストの一覧です。
テストの件数ではなく、どの性質・境界・組み合わせを保証しているかで品質を判断するための資料です。

目標は次の 1 文です。

> どんな入力でも「明確に拒否される」か「性質を満たす結果が返る」かのどちらかになる。旧版と違う箇所は、すべて意図して変えたものである。

バグが見つかったときは、まずこの一覧のどの行が破れていたのかを確かめます。どの行にも当てはまらなければ、
一覧に行を足してから直します（足した行には必ずテストを対応させます）。

## 入力の範囲

すべての計算は `validate::project`（[validate.rs](../crates/mmana-core/src/validate.rs)）を通ります。
`calculate`・周波数特性・遠方界・近傍界・最適化は `solve` から、パルス配置は `pulse_layout` から呼ばれ、
どちらも最初にこの関数で入力を確かめるので、同じ入力が入口によって通ったり拒否されたりすることはありません。
例外は、パルス配置が給電点・負荷・重なったワイヤを確かめないことだけです（編集中でも配置を表示するため）。

範囲は、デスクトップアプリの入力欄の範囲と、エンジンが値に意味を持たせられる範囲に合わせています。
JSON・API・`.maa` の数式（`1e999` は無限大になる）・最適化の試行からは、入力欄の範囲外の値も届きます。

| 項目 | 受け付ける範囲 | 範囲外のとき |
|---|---|---|
| 計算周波数（設計周波数、`options.frequencyMhz`） | 有限で 0 より大きい | `InvalidFrequency` |
| 水平パターンの仰角（`options.horizontalCutElevationDeg`） | −90〜90° | `OutOfRange` |
| パルス数（`options.maxPulses`、最大 8192） | 分割した結果がこの数以下 | `TooManyPulses`（配列を確保する前） |
| ワイヤ | 1 本以上 | `NoWires` |
| ワイヤの半径 | 有限（負の値は組み合わせワイヤの指定、0 は無効なワイヤ） | `OutOfRange`。組み合わせワイヤがなければ `CombinedWireNotFound` |
| R ≠ 0 のワイヤの座標・長さ | 有限。長さが 0 でない | `OutOfRange`、`ZeroLengthWire` |
| R ≠ 0 のワイヤどうし（計算のみ） | 両端が一致するワイヤがない | `OverlappingWires` |
| 自動分割 | DM1・DM2 ≥ 1、SC 1.01〜3、EC ≥ 1 | `InvalidAutoSegmentation` |
| ワイヤの分割数 | 固定分割数 ≥ 1。個別テーパの DM は 1〜32767（DM2 なしの DM1 は 1〜10000） | `InvalidSegmentCount`、`InvalidWireSegmentation` |
| 組み合わせワイヤ | キーが有限。区間の長さ・半径が有限で 0 以上（長さ 0 は区間の終わり、半径 0 は前の区間と同じ） | `OutOfRange` |
| スタック | 横・縦の個数 ≥ 1。個数が 2 以上の向きの間隔が有限 | `OutOfRange` |
| スタックの規模 | 個数 × R ≠ 0 のワイヤ本数 ≤ 4096 | `TooManySegmentWires`（コピーを作る前） |
| 展開ワイヤ（テーパ・組み合わせワイヤで分かれたワイヤ） | 4096 本以下 | `TooManySegmentWires` |
| 給電点（計算のみ） | 1 個以上。電圧・位相が有限。電圧のどれかが 0 でない。名前が解決できる | `NoSources`、`OutOfRange`、`NoExcitation`、`PulseOutOfRange` |
| パルス名 | 番号・ワイヤ番号・オフセットが 2147483647 以下 | 解決できない名前として扱う |
| 負荷（有効なときの計算のみ） | 値が有限。L・C の両方が 0 ではない。S パラメータ形式は 12 項以下で、分母の係数のどれかが 0 でない。名前が解決できる | `OutOfRange`、`InvalidLoad`、`PulseOutOfRange` |
| SWR 基準 | R は有限で 0 より大きい。X は有限 | `OutOfRange` |
| F/B の後方範囲 | 水平 0〜360°、垂直 0〜180° | `OutOfRange` |
| 地上高（大地ありのとき） | 有限 | `OutOfRange` |
| 実大地（実大地のとき） | 媒質が 1 つ以上。誘電率は有限で 0 より大きい。導電率は有限で 0 以上。境界・高さは有限 | `NoGroundMedia`、`OutOfRange` |
| ラジアル（実大地のとき） | 本数 ≥ 0。本数が 1 以上なら半径は有限で 0 より大きい | `OutOfRange` |
| 遠方界・近傍界の軸 | 始点・刻みが有限、個数 ≥ 0（0 と刻み 0 は 1 点） | `OutOfRange` |
| 遠方界・近傍界の点数 | 1,000,000 以下（`MAX_FIELD_POINTS`。桁あふれしない飽和演算で数える） | `TooManyFieldPoints`（確保する前） |
| 近傍界の電力 | 有限で 0 より大きい | `OutOfRange` |
| 周波数特性の点数・指定した点の数 | 1〜100,000（`MAX_SWEEP_POINTS`） | `InvalidSweepPoints`（確保する前） |
| 周波数特性で指定した点 | 点数未満 | `SweepPointOutOfRange` |
| 上の範囲内でも解が有限にならない（間隔 0 のスタック、負荷の共振など） | — | `NoFiniteSolution` |

どの行も、境界上の値が通ること、すぐ外の値が拒否されること、`calculate` と `pulse_layout` の判定を
[input_ranges.rs](../crates/mmana-core/tests/input_ranges.rs) で確かめています。
上限を超えた入力が大きな確保より前に拒否されることは [allocation.rs](../crates/mmana-core/tests/allocation.rs)
（最大の確保が 1 MB 未満であること）で確かめています。

### HTTP API の上限

API は計算を始める前に、サーバーの設定（`mmana_api::Limits`）による上限を確かめます。どれもエンジンの上限以下です
（`server_limits_are_within_the_engine_limits`）。上限を超えると 422 `limit-exceeded`、エンジンが拒否した入力は 422 `calculation-failed` で、
どのエンドポイントに送っても同じ問題として返ります（`an_invalid_input_is_rejected_alike_by_every_endpoint`）。

| 項目 | 既定値 | 超えたとき |
|---|---|---|
| `options.maxPulses` | 4096 | 要求の値をこの値まで下げて計算する（パルス数が超えれば `TooManyPulses`） |
| 周波数特性の点数・指定した点の数 | 401 | `limit-exceeded` |
| 遠方界の方向数・近傍界の点数 | 65,341 | `limit-exceeded` |
| エレメント編集・最適化の設定編集の（操作数 + 1）× ワイヤ数 | 1,000,000 | `limit-exceeded` |
| 最適化 1 回の計算回数 | 2000 | その回数で打ち切り、それまでの最良値を返す |
| リクエストの大きさ | 4 MB | 413 |
