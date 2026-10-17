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
  const [item] = await Promise.all([page.waitForEvent("download"), page.locator(selector).click()]);
  return fs.readFile(await item.path(), "utf8");
}
async function openAnswer() {
  const oldHref = await page.locator("#download-tex").getAttribute("href");
  await page.locator("#solution-panel summary").click();
  await page.waitForFunction(previous => {
    const link = document.getElementById("download-tex");
    return document.getElementById("solution-panel").open && link.href.startsWith("blob:")
      && link.href !== previous && !link.hasAttribute("aria-disabled");
  }, oldHref);
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
  assert.ok(status.families.length > 28);
  assert.ok(status.variants.length > fixtures.length);
  assert.ok(status.methods.length > 10);
  assert.ok(status.coverage.some(c => c.status === "planned"));

  // The actual rendered UI generates and saves without an HTTP API.
  await page.locator("#generate").click();
  await waitUI();
  const firstLabel = await page.locator("#problem-label").textContent();
  assert.equal(await page.locator("#solution-panel").getAttribute("open"), null);
  assert.equal(await page.locator("#problem-family").isVisible(), false);
  assert.match(await page.locator("#verification-label").textContent(), /計算確認済み|指数・剰余 検査済み/);
  const question = await download("#download-tex");
  assert.ok(!question.includes("\\subsection*{解答}"));
  await page.locator("#hint-panel summary").click();
  assert.equal(await download("#download-tex"), question);
  await openAnswer();
  const answer = await download("#download-tex");
  assert.ok(answer.includes("\\subsection*{解答}"));
  await page.locator("#verification-panel summary").click();
  const recipe = await download("#download-recipe");
  assert.equal(typeof JSON.parse(recipe).seed, "string");
  await page.locator("#recipe-file").setInputFiles({ name: "saved.json", mimeType: "application/json", buffer: Buffer.from(recipe) });
  await waitUI();
  assert.equal(await page.locator("#problem-label").textContent(), firstLabel);
  assert.equal(await page.locator("#solution-panel").getAttribute("open"), null);

  // Exercise a known multi-route question through the real import and UI. An
  // alternative method is a valid choice even when it is not the primary route.
  const methodProblem = await resultFor({ level: 2, family: "affine", seed: "42" });
  checked(methodProblem);
  const methodRoutes = methodProblem.problem.routes;
  assert.ok(methodRoutes.filter(route => route.complete).length >= 2);
  assert.equal(new Set(methodRoutes.map(route => route.method_id)).size, methodRoutes.length,
    "The same solution method must not inflate the alternative count");
  await page.locator("#practice-mode").selectOption("method_choice");
  assert.equal(await page.locator("#family").isDisabled(), true);
  assert.equal(await page.locator("#family").inputValue(), "mixed");
  await page.locator("#recipe-file").setInputFiles({ name: "method-practice.json", mimeType: "application/json", buffer: Buffer.from(JSON.stringify(methodProblem.problem.recipe)) });
  await waitUI();
  assert.equal(await page.locator("#method-exercise").isVisible(), true);
  assert.equal(await page.locator("#problem-family").isVisible(), false);
  assert.equal(await page.locator("#solution-content .route-title").isVisible(), false);
  assert.equal(await page.locator("#alternatives-exercise").isVisible(), false);
  assert.equal(await page.locator(".alternative-solution").first().isVisible(), false);
  const methodQuestion = await download("#download-tex");
  assert.ok(!methodQuestion.includes("\\subsection*{解答}"));
  assert.ok(!methodQuestion.includes("\\subsection*{別解"));
  await page.locator("#method-choice").selectOption(methodRoutes[1].method_id);
  await page.locator("#check-method").click();
  assert.match(await page.locator("#method-feedback").textContent(), /この方法で解くことができます/);
  const unlisted = status.methods.find(method => !methodRoutes.some(route => route.method_id === method.id));
  assert.ok(unlisted, "The method selector also offers approaches not listed for this question");
  await page.locator("#method-choice").selectOption(unlisted.id);
  await page.locator("#check-method").click();
  assert.match(await page.locator("#method-feedback").textContent(), /誤りとは限りません/);
  await page.locator("#method-choice").selectOption("custom");
  await page.locator("#check-method").click();
  assert.match(await page.locator("#method-feedback").textContent(), /自分の方針で進めて/);
  await page.locator("#hint-panel summary").click();
  assert.equal(await page.locator("#problem-family").isVisible(), false);
  assert.equal(await page.locator("#alternatives-exercise").isVisible(), false);
  assert.equal(await download("#download-tex"), methodQuestion);
  await openAnswer();
  assert.equal(await page.locator("#alternatives-exercise").isVisible(), true);
  assert.equal(await page.locator(".alternative-solution[open]").count(), 0);
  const alternativeHint = page.locator("#alternatives-content details").filter({ hasText: "別解1の着眼点のヒント" });
  await alternativeHint.locator("summary").click();
  assert.equal(await page.locator(".alternative-solution[open]").count(), 0);
  await page.locator(".alternative-solution").first().locator("summary").click();
  assert.equal(await page.locator(".alternative-solution[open]").count(), 1);
  assert.equal(await page.locator("#solution-panel").getAttribute("open"), "");
  assert.ok((await page.locator(".alternative-solution").first().locator(".route-title").textContent()).includes(methodRoutes[1].method_name));
  assert.match(await page.locator("#save-description").textContent(), /すべての別解/);
  const allRoutesTex = await download("#download-tex");
  for (const [index, route] of methodRoutes.entries()) {
    if (index > 0) assert.ok(allRoutesTex.includes(`\\subsection*{別解${index}：${route.method_name}}`), route.method_name);
    for (const step of route.solution) if (step.latex) assert.ok(allRoutesTex.includes(step.latex), step.latex);
  }
  const alteredRoute = structuredClone(methodProblem.problem);
  alteredRoute.routes[1].solution[0].text += "改変";
  assert.match(await page.evaluate(async problem => {
    try { await window.RecurrenceBrowser.tex(problem, true); return "accepted"; }
    catch (error) { return error.message; }
  }, alteredRoute), /一致しません/);

  // Generation in alternative mode must select a problem with two genuinely
  // complete routes, then keep each answer collapsed until explicitly opened.
  await page.locator("#practice-mode").selectOption("alternatives");
  await page.locator('.level-button[data-level="2"]').click();
  await page.locator("#generate").click();
  await waitUI();
  assert.equal(await page.locator("#method-exercise").isVisible(), false);
  assert.equal(await page.locator("#problem-family").isVisible(), false);
  assert.equal(await page.locator("#alternatives-exercise").isVisible(), false);
  await openAnswer();
  assert.ok(await page.locator(".alternative-solution").count() >= 1);
  assert.equal(await page.locator(".alternative-solution[open]").count(), 0);
  await page.locator("#verification-panel summary").click();
  const alternativeRecipe = JSON.parse(await download("#download-recipe"));
  const alternativeReplay = await resultFor({ recipe: alternativeRecipe }, "replay");
  checked(alternativeReplay);
  assert.ok(alternativeReplay.problem.routes.filter(route => route.complete).length >= 2);
  await page.locator("#practice-mode").selectOption("standard");

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
  const beforeInvalidSeeds = await page.evaluate(() => window.RecurrenceBrowser.request("/api/status"));
  for (const seed of ["18446744073709551616", "1e3", -1, 1.5, 9007199254740992]) {
    const invalidSeed = await resultFor({ level: 1, seed });
    assert.equal(invalidSeed.status, "failed", `Invalid seed accepted: ${seed}`);
    assert.equal(invalidSeed.problem, null);
    assert.match(invalidSeed.message, /seed/);
  }
  const afterInvalidSeeds = await page.evaluate(() => window.RecurrenceBrowser.request("/api/status"));
  assert.equal(afterInvalidSeeds.selection_history_count, beforeInvalidSeeds.selection_history_count,
    "Rejected requests must not change the accepted selection history");
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
  await openAnswer();
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

  // A separate browser profile starts with the previous release's ID-only
  // history. Migration retains duplicate avoidance without inventing variants.
  const migrationContext = await browser.newContext({ offline: !staticPreview });
  const migrationPage = await migrationContext.newPage();
  await migrationPage.addInitScript(id => {
    try {
      localStorage.removeItem("recurrence-lab:browser:recent:v2");
      localStorage.setItem("recurrence-lab:browser:recent:v1", JSON.stringify([id]));
      window.historyStorageAvailable = true;
    } catch (_) { window.historyStorageAvailable = false; }
  }, fixtures[0].id);
  await migrationPage.goto(staticPreview ? `${previewBase}/browser/${encodeURIComponent("漸化式ラボ.html")}` : pathToFileURL(copied).href);
  await migrationPage.waitForFunction(() => !document.getElementById("generate").disabled);
  await migrationContext.setOffline(true);
  const migrated = await migrationPage.evaluate(async () => ({
    available: window.historyStorageAvailable,
    status: await window.RecurrenceBrowser.request("/api/status"),
    records: window.historyStorageAvailable ? JSON.parse(localStorage.getItem("recurrence-lab:browser:recent:v2")) : null,
  }));
  if (migrated.available) {
    assert.equal(migrated.status.history_storage, "local");
    assert.equal(migrated.status.history_count, 1);
    assert.equal(migrated.status.selection_history_count, 1);
    assert.deepEqual(migrated.records, [{ id: fixtures[0].id, family: "", variant: "" }]);
  } else assert.equal(migrated.status.history_storage, "memory");
  await migrationContext.close();
  const summary = { application_version: status.application_version, standalone_file: true, file_navigation_tested: !staticPreview,
    load_method: staticPreview ? "loopback static file then browser network disabled" : "file://", offline: true, http_requests_during_exercises: external.length,
    generated_families: status.families.length, replayed_profiles: fixtures.length, numeric_u64_recipe: "preserved",
    UI: ["generation", "hints", "answer", "method choice", "alternative method accepted", "unlisted method feedback", "independent alternatives", "all-route TeX", "Recipe", "replay", "paired equations", "390px", "1440px"],
    invalid_seeds: "rejected without changing history", altered_solution_route: "rejected", legacy_history_migration: migrated.available ? "preserved" : "storage unavailable; memory fallback",
    invalid_recipe: "rejected", cancellation: "worker terminated", formal_verification: false, javascript_errors: errors };
  await fs.writeFile(path.join(root, "test-results/browser-offline-summary.json"), JSON.stringify(summary, null, 2) + "\n");
  console.log(`PASS: standalone HTML/${staticPreview ? "static preview" : "file://"} offline; ${status.families.length} families; ${fixtures.length} legacy profile native/WASM parity; full-u64 Recipe; exports/hints/answers/cancel; 1440px/390px; zero HTTP requests during exercises`);
} finally {
  await browser.close();
  if (server) await new Promise(resolve => server.close(resolve));
  await fs.rm(temp, { recursive: true, force: true });
}
