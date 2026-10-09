import assert from "node:assert/strict";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import http from "node:http";
import { fileURLToPath, pathToFileURL } from "node:url";
import { chromium } from "playwright-core";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const fixtures = JSON.parse(await fs.readFile(path.join(root, "tests/browser-fixtures.json"), "utf8"));
const temp = await fs.mkdtemp(path.join(os.tmpdir(), "recurrence-standalone-"));
const copied = path.join(temp, "漸化式ラボ.html");
await fs.copyFile(path.join(root, "browser/漸化式ラボ.html"), copied);
// Managed browsers may forbid file navigation. This explicit fallback serves
// static files on allowed loopback, then disables network before any exercises.
const staticPreview = process.env.RECURRENCE_TEST_STATIC_PREVIEW === "1";
let server = null, previewBase = null;
if (staticPreview) {
  server = http.createServer(async (request, response) => {
    try {
      const relative = decodeURIComponent(new URL(request.url, "http://localhost").pathname).replace(/^\/+/, "");
      const source = path.resolve(root, relative);
      if (!source.startsWith(root + path.sep)) throw new Error("Invalid path");
      const mime = source.endsWith(".html") ? "text/html;charset=utf-8" : source.endsWith(".js") ? "application/javascript" : source.endsWith(".css") ? "text/css" : "application/octet-stream";
      const contents = await fs.readFile(source);
      response.writeHead(200, { "Content-Type": mime });
      response.end(contents);
    } catch (_) { response.writeHead(404); response.end(); }
  });
  await new Promise(resolve => server.listen(0, "127.0.0.1", resolve));
  previewBase = `http://127.0.0.1:${server.address().port}`;
}
const browser = await chromium.launch({ executablePath: process.env.CHROMIUM_PATH || "/usr/bin/chromium", headless: true, args: ["--no-sandbox"] });
const context = await browser.newContext({ viewport: { width: 1440, height: 1100 }, offline: !staticPreview, acceptDownloads: true });
const page = await context.newPage();
const errors = [], external = [];
page.on("pageerror", e => errors.push(e.message));
page.on("console", message => { if (message.type() === "error") errors.push(message.text()); });
page.on("request", request => { if (/^https?:/i.test(request.url())) external.push(request.url()); });
await fs.mkdir(path.join(root, "test-results"), { recursive: true });

async function resultFor(request, operation = "generate") {
  return page.evaluate(async ({ request, operation }) => {
    const runtime = window.RecurrenceBrowser;
    const started = await runtime.request(`/api/${operation}`, { method: "POST", body: JSON.stringify(request) });
    const deadline = Date.now() + 12000;
    while (Date.now() < deadline) {
      const job = await runtime.request(`/api/jobs/${started.id}`);
      if (["succeeded", "failed", "timeout", "cancelled"].includes(job.status)) return job;
      await new Promise(resolve => setTimeout(resolve, 20));
    }
    throw new Error("Browser generation did not finish");
  }, { request, operation });
}
function checked(job) {
  assert.equal(job.status, "succeeded", job.message);
  assert.equal(job.verification.status, "browser_checked");
  assert.equal(job.verification.formal_verification, false);
  assert.equal(job.verification.lean_executed, false);
  assert.equal(job.verification.checked_terms, 20);
  assert.equal(typeof job.problem.recipe.seed, "string");
}
async function waitUI() {
  await page.locator("#job-progress").waitFor({ state: "visible", timeout: 12000 });
  await page.locator("#job-progress").waitFor({ state: "hidden", timeout: 12000 });
  await page.locator("#problem-content").waitFor({ state: "visible" });
  await page.waitForFunction(() => document.getElementById("download-tex").href.startsWith("blob:"));
  assert.equal(await page.locator("#notice").isVisible(), false, await page.locator("#notice-text").textContent());
}
async function download(selector) {
  const event = page.waitForEvent("download");
  await page.locator(selector).click();
  const item = await event;
  return fs.readFile(await item.path(), "utf8");
}

try {
  await page.goto(staticPreview ? `${previewBase}/browser/${encodeURIComponent("漸化式ラボ.html")}` : pathToFileURL(copied).href);
  await page.waitForFunction(() => !document.getElementById("generate").disabled);
  await context.setOffline(true);
  external.length = 0;
  assert.equal(await page.locator("#deployment-label").textContent(), "このブラウザーで実行");
  assert.equal(await page.locator(".brand").getAttribute("href"), "#", "Standalone navigation must not require sibling files");
  assert.equal(await page.locator("#setup-help").isVisible(), false);
  const status = await page.evaluate(() => window.RecurrenceBrowser.request("/api/status"));
  assert.equal(status.ready, true);
  assert.equal(status.deployment_mode, "browser");
  assert.equal(status.families.length, 28);

  // The actual rendered UI generates and saves without an HTTP API.
  await page.locator("#generate").click();
  await waitUI();
  const firstLabel = await page.locator("#problem-label").textContent();
  assert.equal(await page.locator("#solution-panel").getAttribute("open"), null);
  assert.equal(await page.locator("#problem-family").isVisible(), false);
  assert.match(await page.locator("#verification-label").textContent(), /計算確認済み/);
  const question = await download("#download-tex");
  assert.ok(!question.includes("\\subsection*{解答}"));
  await page.locator("#hint-panel summary").click();
  assert.equal(await download("#download-tex"), question);
  await page.locator("#solution-panel summary").click();
  await page.waitForFunction(() => !document.getElementById("download-tex").hasAttribute("aria-disabled"));
  const answer = await download("#download-tex");
  assert.ok(answer.includes("\\subsection*{解答}"));
  await page.locator("#verification-panel summary").click();
  const recipe = await download("#download-recipe");
  assert.equal(typeof JSON.parse(recipe).seed, "string");
  await page.locator("#recipe-file").setInputFiles({ name: "saved.json", mimeType: "application/json", buffer: Buffer.from(recipe) });
  await waitUI();
  assert.equal(await page.locator("#problem-label").textContent(), firstLabel);
  assert.equal(await page.locator("#solution-panel").getAttribute("open"), null);

  for (const family of status.families) {
    const job = await resultFor({ level: family.levels[0], family: family.id, seed: "18446744073709551615" });
    checked(job);
    assert.equal(job.problem.recipe.family, family.id);
    assert.equal(job.problem.classification.level, family.levels[0]);
  }
  for (const fixture of fixtures) {
    const job = await resultFor({ recipe: fixture.recipe }, "replay");
    checked(job);
    for (const key of ["id", "statement_latex", "answer_latex", "classification"]) {
      assert.deepEqual(job.problem[key], fixture[key], `Native/WASM parity: ${fixture.recipe.family}, ${key}`);
    }
  }

  // A native numeric u64 Recipe travels as original JSON text and is never rounded.
  const legacy = JSON.stringify(fixtures[0].recipe).replace(/"seed":"\d+"/, '"seed":18446744073709551615');
  const preserved = await resultFor({ recipe: legacy }, "replay");
  checked(preserved);
  assert.equal(preserved.problem.recipe.seed, "18446744073709551615");
  assert.equal(preserved.problem.id, fixtures[0].id);
  const bad = { ...fixtures[0].recipe, output: "tampered" };
  const rejected = await resultFor({ recipe: bad }, "replay");
  assert.equal(rejected.status, "failed");
  assert.equal(rejected.problem, null);
  const cancelled = await page.evaluate(async () => {
    const runtime = window.RecurrenceBrowser;
    const job = await runtime.request("/api/generate", { method: "POST", body: JSON.stringify({ level: 5, seed: "1" }) });
    await runtime.request(`/api/jobs/${job.id}/cancel`, { method: "POST" });
    return runtime.request(`/api/jobs/${job.id}`);
  });
  assert.equal(cancelled.status, "cancelled");
  assert.equal(cancelled.problem, null);

  // Load a long paired question through the real file input for layout and export.
  const pair = fixtures.find(f => f.recipe.family === "coupled_scaled");
  await page.locator("#recipe-file").setInputFiles({ name: "paired.json", mimeType: "application/json", buffer: Buffer.from(JSON.stringify(pair.recipe)) });
  await waitUI();
  await page.locator("#solution-panel summary").click();
  await page.waitForFunction(() => !document.getElementById("download-tex").hasAttribute("aria-disabled"));
  assert.equal(await page.locator(".katex-error").count(), 0);
  await page.locator("#verification-panel summary").click();
  assert.match(await page.locator("#verification-metadata").textContent(), /Lean実行していません/);
  for (const width of [1440, 390]) {
    await page.setViewportSize({ width, height: width === 390 ? 844 : 1100 });
    const size = await page.evaluate(() => ({ actual: document.documentElement.scrollWidth, viewport: innerWidth }));
    assert.ok(size.actual <= size.viewport + 1, JSON.stringify(size));
    await page.screenshot({ path: path.join(root, `test-results/browser-offline-${width}.png`), fullPage: true });
  }
  assert.deepEqual(errors, []);
  assert.deepEqual(external, []);

  const folder = await context.newPage();
  if (staticPreview) await context.setOffline(false);
  await folder.goto(staticPreview ? `${previewBase}/browser/index.html` : pathToFileURL(path.join(root, "browser/index.html")).href);
  await folder.waitForFunction(() => !document.getElementById("generate").disabled);
  const launcher = await context.newPage();
  await launcher.goto(staticPreview ? `${previewBase}/index.html` : pathToFileURL(path.join(root, "index.html")).href);
  await launcher.waitForFunction(() => !document.getElementById("generate").disabled);
  const summary = { application_version: "0.3.0", standalone_file: true, file_navigation_tested: !staticPreview,
    load_method: staticPreview ? "loopback static file then browser network disabled" : "file://", offline: true, http_requests_during_exercises: external.length,
    generated_families: status.families.length, replayed_profiles: fixtures.length, numeric_u64_recipe: "preserved",
    UI: ["generation", "hints", "answer", "TeX", "Recipe", "replay", "paired equations", "390px", "1440px"],
    invalid_recipe: "rejected", cancellation: "worker terminated", formal_verification: false, javascript_errors: errors };
  await fs.writeFile(path.join(root, "test-results/browser-offline-summary.json"), JSON.stringify(summary, null, 2) + "\n");
  console.log(`PASS: standalone HTML/${staticPreview ? "static preview" : "file://"} offline; 28 families; 75 profile native/WASM parity; full-u64 Recipe; exports/hints/answers/cancel; 1440px/390px; zero HTTP requests during exercises`);
} finally {
  await browser.close();
  if (server) await new Promise(resolve => server.close(resolve));
  await fs.rm(temp, { recursive: true, force: true });
}
