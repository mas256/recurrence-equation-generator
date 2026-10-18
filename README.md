# 漸化式ラボ

**HTMLをブラウザーで開くだけで使える、漸化式の演習アプリです。** 各利用者のブラウザー内でRust/WebAssemblyが問題を動的に生成します。サーバー、アカウント、RustやLeanのインストールは不要です。数式表示とフォントも同梱し、オフラインで使用できます。

v0.4.0は42系統・123細分類です。例外係数・退化・重根を含む細分類、解法選択の演習、途中式付きの別解、履歴に基づく出題配分を追加しました。対応範囲と未実装の枝は画面の「生成範囲」と[改修設計](docs/exercise-redesign.md)で確認できます。有限個の登録規則で、解ける漸化式全体を網羅したとは扱いません。

## すぐに使う

1. このリポジトリをGitHubの「Code → Download ZIP」からダウンロードし、展開します。
2. 展開したフォルダーの **[index.html](index.html)** をブラウザーで開きます。ブラウザー版の画面に移動します。
3. 難易度と必要に応じて型を選び、「新しい問題をつくる」を押します。

**[browser/漸化式ラボ.html](browser/漸化式ラボ.html)** を直接開いても使用できます。このHTMLは生成エンジン、数式表示、フォントをすべて含むため、1ファイルだけを別のPCに渡せます。GitHub上のコード表示画面では実行されません。HTMLをダウンロードして開いてください。

複数ファイルに分けた版は[browser/index.html](browser/index.html)です。この版を使う場合は`browser`フォルダー全体を保持してください。動作環境、保存と再現、開発用のビルド手順は[ブラウザー版の説明](docs/browser.md)に記載しています。

## 使い方

1. 難易度と任意の型を選び、「新しい問題をつくる」を押します。総合演習では型を解答まで伏せます。
2. 通常演習・解法選択・別解演習を選べます。解法選択では、登録されている複数の正しい方針を受け付けます。未登録の解法は誤りと断定せず、模範解答との比較を促します。
3. 「考え方のヒント」「解答をたしかめる」は個別に開けます。生成中は「生成を中止する」で処理を終了できます。
4. 「TeXを保存」は、解答を閉じているときは問題のみ、開いているときは主解法と登録済み別解の途中式も保存します。
5. 詳細欄の「Recipeを保存」で構成をJSONファイルに保存できます。「保存したRecipeから問題を再現」で同じ問題を再構成します。

TeXとRecipeはブラウザーからファイルとしてダウンロードします。問題はページを閉じると失われるため、後で使う問題はRecipeを保存してください。生成の失敗や中止時は、すでに表示した問題を保持します。

TeXはUTF-8のjsbook、A4、11pt、20mm余白、2段組、段間12pt、罫線0.4ptです。PDF化には利用者のpLaTeXとdvipdfmxを使用します。

## 数学と確認の範囲

|難易度|対応する型|
|---|---|
|Lv1|定数、初項だけ異なる定数尾、等差・等比、一次式の階差|
|Lv2|不動点、階比・階乗、二次式の階差、定数の二階差、退化した3項間|
|Lv3|基本3項間・重根、指数係数の階比、一次式の特解、連立の分離、総和の消去、逆数型|
|Lv4|一次分数、倍率の正規化、定数項を含む3項間、正規化する連立|
|Lv5|逆数と変数係数、階差と総和、部分和の正規化、対数と二重階差、一次付加項のある連立|

Rustの任意精度整数と既約有理数を使い、問題・解法・TeXを共通の型付きデータから作ります。Recipeの版、パラメータ、ブロック接続を検査し、再生成した内容との一致を確認します。旧規則版1.0/1.1の保存済みRecipeは従来の問題IDと分類を保持します。新規生成は規則版2.0です。

通常の問題は初期条件・漸化式・定義域をn=20まで厳密計算で検査します。指数が急増する対数型は、小さい項の厳密計算と指数の関係、剰余の検査を組み合わせます。別解の補助数列や途中式も有限項で検査し、未解決の式があれば記録に残します。**ブラウザー版はLeanを実行しません。有限項の確認は全項の形式証明ではありません。** ネイティブ版のLeanによる原問題の証明と、別解の途中式の検査も区別します。

難易度は問題文から構成でき、必須式を有限項で確認できた登録解法の最小スコアで決めます。計算量には別の上限を設けます。スコアは教育上の設計値であり、学習者の実測による校正は今後の課題です。

総合演習では、同じLvの受理済み履歴から不足する大分類、次に細分類を優先します。選択用の履歴は直近2,000件、直近40件の同一問題は除外します。それ以前も2,000件の履歴で未出題を優先し、探索予算内で見つからない場合だけ古い問題の再出題を許します。係数の品質判定中は細分類を固定します。数値の全組合せが等確率になる方式ではありません。

## 任意のRust＋Lean版

ブラウザー単独版とは別に、各問題をLean 4.19.0で検証するローカルアプリも残しています。Linuxで動作確認しています。macOSの準備分岐は実機未確認です。WindowsではWSL2のLinux環境を使用してください。Git、Bash、curl、tar、zstdとRustが必要です。

```bash
bash scripts/setup-lean.sh
bash scripts/start.sh
```

**http://127.0.0.1:3210/** をブラウザーで開きます。Rust未導入の場合は[rustup](https://rustup.rs/)で導入してください。初回は固定したLean・mathlibの取得とRustのビルドに通信が必要です。準備後はローカルで実行できます。

Leanの配布キャッシュを取得できない場合は、必要なmathlibモジュールをソースから構築します。最初からソース構築を指定する場合:

```bash
RECURRENCE_BUILD_FROM_SOURCE=1 bash scripts/setup-lean.sh
```

リリースビルドと設定の例:

```bash
cargo build --release --locked
bash scripts/start.sh --check
bash scripts/start.sh --port 3211 --lean-timeout 90 --request-timeout 180
```

画面の資産は実行ファイルに埋め込まれます。実行ファイルを移動する場合は、準備済みの`lean`フォルダーも同じフォルダーに置くか、`--lean-dir`で指定します。Linux x86_64向け配布実行ファイルはglibc 2.39以上を使用します。以前のglibcではその環境でRustからビルドしてください。

ネイティブ版は、初期条件、全添字の漸化式、必要な定義域条件、任意の解の一意性をLeanで検証します。必須4定理の`#print axioms`を監査し、`propext`、`Classical.choice`、`Quot.sound`以外の依存を拒否します。検証記録は`data/problems/<内容ハッシュ>.json`、証明ソースとログは`lean/generated/<要求別フォルダー>/`に保存します。証明・問題・環境のハッシュ、版、終了コード、所要時間を対応付け、準備した環境が変わった場合は再準備を必要とします。

同時検証は1件で、接続は127.0.0.1を使用します。初期値は候補数200、探索10秒、Lean60秒、要求全体120秒、検証候補2件です。`--candidate-limit`、`--search-timeout`、`--lean-timeout`、`--request-timeout`、`--lean-candidates`で変更できます。保存済みの規則版1.0の重み付き総和Recipeは、再現互換のため数値コスト8を超えてもネイティブ版で再現できます。

## 開発と確認

ブラウザー版のビルド:

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.105 --locked
python3 scripts/build-browser.py
```

Python 3.11以上を使用します。ブラウザー版のフォルダー、単一HTML、`dist/recurrence-lab-browser.zip`を作成します。ビルドツールは利用者の実行時には不要です。

```bash
cargo test --locked
cargo test --locked --features browser --lib
cargo clippy --all-targets -- -D warnings
npm ci
npm run test:offline
```

オフライン画面テストはビルド済みブラウザー版とChromiumを使用します。`CHROMIUM_PATH`でブラウザーの実行ファイルを指定できます。ネイティブ版の画面テストは起動済みアプリに対して`npm run test:browser`を実行します。Node.jsとnpmは開発用の画面テストにのみ使用します。

Lean環境を準備した後の検証:

```bash
cargo test --locked --test lean -- --ignored --nocapture
cargo test --locked --lib -- --ignored --nocapture --test-threads 1
bash scripts/benchmark.sh http://127.0.0.1:3210 10 test-results/benchmark
```

通常テストは厳密な式計算、再現、内容変更、前向きの解法検出と採点、公理監査、教材出力を確認します。Leanテストは実際の全項証明と、時間切れ・中止・改変の拒否を確認します。ベンチマークは各Lv10問を生成して要求・検証時間を記録します（curlとjqが必要）。結果は[検証記録](docs/validation.md)にまとめています。

## 構成と参照元

`src/math.rs`が厳密数式、`src/generator.rs`と`src/extensions/`がRecipeと生成・解法検出・採点、`src/export.rs`がTeXです。`src/browser.rs`はWebAssemblyへの入口と有限項の厳密検査、`web/browser-runtime.js`はブラウザー内の処理管理、`scripts/build-browser.py`はオフライン版の構築を担当します。`src/verifier.rs`と`src/proofs/`はLean接続と証明生成、`src/main.rs`は任意のネイティブ版のローカルAPIです。

参照した数学規則とLean共通定理は[mas256/math-textbooks](https://github.com/mas256/math-textbooks/tree/a21c3d0711ebb08d0516e646f32ee3cab7b2c7de/recurrence)のコミット`a21c3d0711ebb08d0516e646f32ee3cab7b2c7de`です。KaTeXのライセンスは`web/vendor/KATEX-LICENSE`とブラウザー配布物に同梱しています。実装範囲と残る設計事項は[実装状況](docs/implementation-status.md)に記載しています。
