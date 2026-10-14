# 公開ページからの再生成依頼

Cloudflare Workers の無料プランで、[公開ページ](https://mas256.github.io/math-textbooks/recurrence/)から GitHub にログインせずに問題バンクの再生成を依頼できます。このリポジトリは中継 API を管理し、実際に問題バンクを作って公開する `mas256/math-textbooks` の `pages.yml` を起動します。Rust/WebAssembly のブラウザー版は、この公開バンクとは別のアプリです。

処理は `ページの依頼ボタン → Worker → GitHub workflow_dispatch → 既存の生成・Lean検証・画面検査 → Pages公開` です。生成設定を変えなければ同じ問題が作られます。依頼は全利用者で1時間に1回までで、既に公開処理が実行中ならその実行を案内します。SQLite の Durable Object に受付を保存するため、同時クリックや Worker の再起動でも制限を維持します。

## 初回の設定

1. `math-textbooks` 側のページ・ワークフロー変更と、この API の変更をそれぞれ `main` にマージします。API URL 未設定の間、ページの依頼ボタンは「準備中」になります。
2. [Cloudflare](https://dash.cloudflare.com/sign-up) でアカウントを作り、Workers の **Free** プランを利用します。Workers & Pages で `workers.dev` のサブドメインを設定します。独自ドメインは不要です。アカウント ID を確認し、対象アカウントに限定した「Edit Cloudflare Workers」テンプレートの API トークンを作ります。
3. GitHub の [fine-grained personal access token](https://github.com/settings/personal-access-tokens/new) を作り、対象リポジトリを **`mas256/math-textbooks` のみ**、権限を **Actions: Read and write** にします。問題バンクの生成や公開は、起動された workflow 自身の `GITHUB_TOKEN` と Pages 権限で行います。
4. [このリポジトリの Actions secrets](https://github.com/mas256/recurrence-equation-generator/settings/secrets/actions) に次の3つを登録します。値はチャットやソースに貼り付けず、GitHub の設定画面に入力してください。

   | Secret | 値 |
   | --- | --- |
   | `CLOUDFLARE_API_TOKEN` | 手順2で作成した Cloudflare API トークン |
   | `CLOUDFLARE_ACCOUNT_ID` | Cloudflare のアカウント ID |
   | `REGENERATION_GITHUB_TOKEN` | 手順3で作成した GitHub トークン |

5. [Deploy recurrence regeneration API](https://github.com/mas256/recurrence-equation-generator/actions/workflows/deploy-regeneration-api.yml) を `main` で手動実行します。テスト後に Worker と SQLite Durable Object を公開し、GitHub トークンを Worker の Secret `GITHUB_TOKEN` に保存します。サマリーの公開 URL をコピーします。
6. [math-textbooks の Actions variables](https://github.com/mas256/math-textbooks/settings/variables/actions) に **`RECURRENCE_REGENERATION_API_URL`** を作り、手順5の URL（`https://recurrence-regeneration-api.<サブドメイン>.workers.dev`）を設定します。パス・クエリ・認証情報は含めません。
7. [math-textbooks の Deploy Pages](https://github.com/mas256/math-textbooks/actions/workflows/pages.yml) を `main` で手動実行します。ページに API URL が反映され、ボタンが有効になります。
8. ページの「問題バンクの再生成を依頼」を押し、受付 → Actions実行 → 公開完了が表示されることを確認します。公開完了後にページを再読み込みして問題バンクを取得します。

GitHub トークンには有効期限を設定し、期限が近づいたら Secret を更新して API 公開 workflow を再実行します。Cloudflare 側の設定がまだない場合、API を本番で起動することはできません。設定するまで既存の出題機能を利用できます。

## コストと受付制限

[Workers Free](https://developers.cloudflare.com/workers/platform/pricing/) は1日10万リクエストまで、[SQLite Durable Objects](https://developers.cloudflare.com/durable-objects/platform/pricing/) も無料プランで利用できます。小規模な利用では無料枠に収まる見込みです。無料プランの上限を超えると処理が制限されるため、有料プランへの変更は利用者が判断してください。[公開リポジトリの標準 GitHub ホストランナー](https://docs.github.com/en/billing/concepts/product-billing/github-actions) は無料です。

API は匿名公開を意図したもので、CORS はブラウザーからの呼び出し元を限定するだけです。API 自体に GitHub の任意操作機能はなく、起動先は固定した `pages.yml`・`main` のみです。サイト全体の受付を1時間に1回に制限し、状態取得の GitHub API 呼び出しは全訪問者で15秒間共有します。大きなリクエスト本文は受け取りません。通信エラーで受付結果が不明な場合も制限を保持し、二重起動を避けます。1時間後に再び依頼できます。

## 開発・確認

```bash
node --test api/regeneration/worker.test.mjs
npx --yes wrangler@4.150.0 deploy --dry-run --config api/regeneration/wrangler.toml
```

テストは受付の同時実行、再起動後の制限、既存実行の追跡、失敗時の応答、ステータス取得の共有を確認します。本物の Actions は起動しません。

API の `/status` は GET、`/regenerate` は本文なしの POST です。`Origin: https://mas256.github.io` を要求します。ブラウザー側にトークンは渡しません。
