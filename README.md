# 漸化式ラボ

Rustで漸化式をその場で生成し、Lean 4.19.0で検証した一問をブラウザーに表示するローカルアプリです。設計書「漸化式動的生成アプリ Rust実装とLean検証の設計書」を参考に作成しました。

**v0.2.0はLv1〜Lv5・28系統、75種類の登録プロファイル** に対応します。係数と登録した構成から問題を動的に生成します。75は倍率・付加項・指数などの構成の種類数であり、出題する問題数の上限ではありません。参照版v0.7.0の全プロファイルや任意のブロック合成への対応は含みません。

|難易度|対応する型|
|---|---|
|Lv1|等差・等比、一次式の階差|
|Lv2|不動点、階比・階乗、二次式の階差、部分和の関係|
|Lv3|基本3項間、指数係数の階比、一次式の特解、連立の分離|
|Lv4|一次分数、倍率の正規化、定数項を含む3項間、正規化する連立|
|Lv5|逆数と変数係数、階差と総和、部分和の正規化、対数と二重階差、一次付加項のある連立|

各系統の倍率や付加項の範囲は [実装状況](docs/implementation-status.md) に記載しています。初版v0.1.0で保存した7系統のRecipeは、規則版1.0と内容IDを保持して再現できます。追加した21系統は規則版1.1を使用します。

## 起動

Linuxで動作確認しています。macOS向けのLean準備分岐も用意していますが、実機では未確認です。WindowsではWSL2のLinux環境を使用してください。必要なものはGit、Bash、curl、tar、zstdです。ソースからビルドする場合はRustも必要です。Linux x86_64向けの配布一式には実行ファイルを同梱しています。Leanとmathlibは準備スクリプトが固定した版を導入します。

同梱のLinux x86_64実行ファイルはglibc 2.39以上を使用します。以前のglibcの環境では、Rustで`cargo build --release --locked`を実行し、`./target/release/recurrence-lab`を直接起動してください。

アプリのフォルダーで次を実行します。

```bash
bash scripts/setup-lean.sh
bash scripts/start.sh
```

**http://127.0.0.1:3210/** をブラウザーで開きます。Rust未導入の場合は [rustup](https://rustup.rs/) で導入してください。初回のLean準備とRustのビルドには通信が必要です。準備後のアプリ実行はPythonや外部サーバーを必要としません。数式表示のKaTeXとフォントは同梱しています。

Leanの配布キャッシュを取得できない環境では、必要なmathlibモジュールをソースから構築します。初回は時間がかかります。最初からソース構築を指定する場合:

```bash
RECURRENCE_BUILD_FROM_SOURCE=1 bash scripts/setup-lean.sh
```

Rustのリリースビルドを利用する場合:

```bash
cargo build --release --locked
bash scripts/start.sh
```

画面の資産は実行ファイルに埋め込まれます。実行ファイルを移動する場合は、準備済みの`lean`フォルダーも同じフォルダーに置くか、`--lean-dir`で指定します。

## 使い方

1. 難易度と任意の型を選び、「新しい問題をつくる」を押します。
2. 候補の選別とLean検証の進行を表示します。「生成を中止する」で実行中のプロセスも終了します。
3. 「考え方のヒント」「解答をたしかめる」は個別に開けます。総合演習では型を解答まで伏せます。
4. 「TeXを保存」は、解答を閉じているときは問題のみ、開いているときは方針と解答も保存します。TeXはUTF-8のjsbook、A4、11pt、20mm余白、2段組、段間12pt、罫線0.4ptです。PDF化には利用者のpLaTeXとdvipdfmxを使用します。
5. 詳細欄の「Recipeを保存」で構成を保存できます。「保存したRecipeから問題を再現」で同じ内容を再構成し、現在の環境で改めてLean検証します。

準備が未完了、証明失敗、時間切れ、中止の問題には「検証済み」を表示しません。新しい要求の失敗や中止時も、すでに表示した問題を保持します。

## 数学と検証

任意精度の整数と既約の有理数を使用します。画面、解法文、TeX、Leanの命題は型付きのProblemIRとDerivationIRから作成します。数列の値域は有理数とし、学校の添字1をLean内部の添字0に対応させます。

各問題について、一般項の初期条件、すべての添字での漸化式、必要な分母非零条件、任意の解の一意性を検証します。有限項の一致をLeanの証明の代わりには使いません。必須の定理の`#print axioms`を監査し、Leanの標準公理`propext`、`Classical.choice`、`Quot.sound`以外の依存を拒否します。

検証記録は`data/problems/<内容ハッシュ>.json`、独立した証明ソースとログは`lean/generated/<要求別フォルダー>/`に保存します。証明・問題・環境のハッシュ、版、終了コード、所要時間を対応付けます。共通モジュールなどが準備時から変わった場合は再準備が必要です。保存した過去の成功記録を別の内容や版に流用しません。

Recipeは登録した非循環のブロック接続と一致するかを確認します。現版では登録した構成だけを有効にし、任意のブロック編集は受け付けません。難易度は表示する漸化式の係数と初期条件から検出した解法に基づき、既存v0.7.0の教育スコアで判定します。操作・発見・数値・定義域・式の複雑さの内訳も記録します。学習者の実測に基づく分類ではありません。

数値コストは表示式・初期条件と、登録検出器が算出する中間値から測定し、各型の登録範囲で適用します。新規生成はコスト8以下の候補を採用します。互換性のため、保存済みの規則版1.0の重み付き総和Recipeは、この予算を超えても再現できます。数学的な成立条件とLean検証は再現時にも確認します。

## 設定と確認

```bash
bash scripts/start.sh --check
bash scripts/start.sh --port 3211 --lean-timeout 90 --request-timeout 180
```

候補数200、探索10秒、Lean60秒、要求全体120秒、検証候補2件を初期値とします。`--candidate-limit`、`--search-timeout`、`--lean-timeout`、`--request-timeout`、`--lean-candidates`で変更できます。同時検証は1件で、接続は127.0.0.1だけを使用します。

```bash
cargo test --locked
cargo test --locked --test lean -- --ignored --nocapture
cargo test --locked --lib -- --ignored --nocapture --test-threads 1
bash scripts/benchmark.sh http://127.0.0.1:3210 10 test-results/benchmark
```

通常テストは厳密な式計算、再現、内容変更、前向きの解法検出と採点、公理監査、教材出力を確認します。Leanテストは準備後に28系統の実際の全項証明、各拡張モジュールの登録プロファイル、および時間切れ・中止・改変の拒否を確認します。ベンチマークは各Lv10問を順に生成し、結果、環境、検証時間、中央値・95パーセンタイル・最大値を保存します（curlとjqが必要）。実施した検証と実測結果は [検証記録](docs/validation.md) にまとめています。

画面操作の確認は、Chromiumを導入した開発環境で`npm ci`、`npm run test:browser`を実行します。`CHROMIUM_PATH`でブラウザーの実行ファイルを指定できます。npmとNode.jsは開発用の画面テストにのみ使用します。

## 構成と参照元

`src/math.rs`が厳密数式、`src/generator.rs`と`src/extensions/`がRecipeと生成・解法検出・採点、`src/verifier.rs`と`src/proofs/`がLean接続と証明生成、`src/export.rs`がTeX、`src/main.rs`がローカルAPIと要求管理、`web/`が操作画面です。

参照した数学規則とLean共通定理は [mas256/math-textbooks](https://github.com/mas256/math-textbooks/tree/a21c3d0711ebb08d0516e646f32ee3cab7b2c7de/recurrence) のコミット`a21c3d0711ebb08d0516e646f32ee3cab7b2c7de`です。KaTeXのライセンスは`web/vendor/KATEX-LICENSE`に同梱しています。実装範囲と残る設計事項は[docs/implementation-status.md](docs/implementation-status.md)に記載しています。
