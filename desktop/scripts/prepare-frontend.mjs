// The desktop app shows the same self-contained page as browser/漸化式ラボ.html.
// It already embeds the WebAssembly engine, KaTeX and fonts, so the app needs
// no network access and no Rust commands of its own.
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const desktop = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const source = path.join(desktop, "..", "browser", "漸化式ラボ.html");
const dist = path.join(desktop, "dist");
fs.rmSync(dist, { recursive: true, force: true });
fs.mkdirSync(dist, { recursive: true });
// The page describes itself as running in a browser; in the app it runs on this PC.
// Each phrase must match exactly once so that upstream wording changes stop the build.
const wording = [
  ['"このブラウザーで実行"', '"このPCで実行"'],
  ['"ブラウザー内で生成"', '"PC内で生成"'],
  ['"ブラウザーの準備"', '"アプリの準備"'],
  ['["問題は、このブラウザー内で生成。", "HTMLを開くだけで演習を始められます。"]',
    '["問題は、このPC内で生成。", "インストールしたアプリだけで演習を始められます。"]'],
  ['"ブラウザー版は有理数による有限範囲', '"アプリ版は有理数による有限範囲'],
  ['"問題の生成と計算の確認は、このブラウザー内で行います。"', '"問題の生成と計算の確認は、このPC内で行います。"'],
  ['["実行", "このブラウザー内"]', '["実行", "このPC内（アプリ）"]'],
  ['"ブラウザー内でRecipeと最初の20項を確認しています。"', '"PC内でRecipeと最初の20項を確認しています。"'],
  ['"生成エンジンをブラウザー内で利用できます。', '"生成エンジンをこのPC内で利用できます。'],
];
let page = fs.readFileSync(source, "utf8");
for (const [from, to] of wording) {
  const count = page.split(from).length - 1;
  if (count !== 1) throw new Error(`Expected one occurrence of ${from}, found ${count}`);
  page = page.replace(from, to);
}
fs.writeFileSync(path.join(dist, "index.html"), page);
console.log(`Prepared ${path.relative(desktop, source)} -> dist/index.html`);
