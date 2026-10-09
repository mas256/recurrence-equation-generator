import assert from "node:assert/strict";
import fs from "node:fs/promises";

const { chromium } = await import(process.env.PLAYWRIGHT_MODULE_PATH || "playwright-core");
const base = (process.env.RECURRENCE_TEST_URL || "http://127.0.0.1:3210").replace(/\/$/, "");
const ready = await (await fetch(`${base}/api/status`)).json();
assert.equal(ready.ready, true, "Prepare Lean before browser verification");
assert.equal(ready.application_version, "0.2.0", "Expanded UI checks require the rebuilt 0.2 application");
assert.equal(ready.families.length, 28, "All 28 reference families must be available");
assert.equal(ready.active_job, null, "Run UI verification separately from benchmarks");
const browser = await chromium.launch({
  executablePath: process.env.CHROMIUM_PATH || "/usr/bin/chromium", headless: true, args: ["--no-sandbox"],
});
const page = await browser.newPage({ viewport: { width: 1440, height: 1100 } });
const errors = [], external = [], consoleErrors = [];
page.on("pageerror", (error) => errors.push(error.message));
page.on("console", (message) => { if (message.type() === "error") consoleErrors.push(message.text()); });
page.on("request", (request) => { if (!request.url().startsWith(base)) external.push(request.url()); });
await fs.mkdir("test-results", { recursive: true });
await fs.mkdir("docs", { recursive: true });

async function waitGenerated() {
  await page.locator("#job-progress").waitFor({ state: "visible", timeout: 15000 });
  await page.locator("#job-progress").waitFor({ state: "hidden", timeout: 125000 });
  assert.equal(await page.locator("#notice").isVisible(), false, await page.locator("#notice-text").textContent());
  await page.locator("#problem-content").waitFor({ state: "visible" });
  assert.equal(await page.locator("#solution-panel").getAttribute("open"), null);
  assert.equal(await page.locator(".katex-error").count(), 0);
}
async function jobResult(id) {
  const response = await fetch(`${base}/api/jobs/${id}`);
  assert.equal(response.ok, true);
  const job = await response.json();
  assert.equal(job.status, "succeeded", job.message);
  assert.equal(job.verification.status, "success");
  assert.equal(job.verification.problem_hash, job.problem.id);
  return job;
}
async function generateFamily(level, family) {
  await page.locator(`[data-level="${level}"]`).click();
  await page.locator("#family").selectOption(family);
  const accepted = page.waitForResponse((r) => r.url().endsWith("/api/generate") && r.status() === 202);
  await page.locator("#generate").click();
  const job = await (await accepted).json();
  await waitGenerated();
  const result = await jobResult(job.id);
  assert.equal(result.problem.recipe.family, family);
  assert.equal(result.problem.classification.level, level);
  return result;
}
async function openAnswer() {
  await page.locator("#solution-panel summary").click();
  await page.waitForFunction(() => document.getElementById("download-tex").href.endsWith("solution=true"));
  assert.equal(await page.locator(".answer-box").isVisible(), true);
}
async function annotation(selector) {
  return page.locator(`${selector} annotation[encoding="application/x-tex"]`).textContent();
}
async function downloadedTex() {
  const href = await page.locator("#download-tex").getAttribute("href");
  const response = await fetch(new URL(href, base));
  assert.equal(response.ok, true);
  return response.text();
}
function gatheredRows(latex) {
  const content = latex.match(/\\begin\{gathered\}([\s\S]*?)\\end\{gathered\}/);
  assert.ok(content, "Expected a gathered equation group");
  return content[1].split("\\\\").map((row) => row.replace("\\quad(n\\ge1)", ""));
}
async function assertResponsive(label) {
  for (const viewport of [{ width: 1440, height: 1100 }, { width: 390, height: 844 }]) {
    await page.setViewportSize(viewport);
    const size = await page.evaluate(() => ({ actual: document.documentElement.scrollWidth, viewport: innerWidth }));
    assert.ok(size.actual <= size.viewport + 1, `${label}, ${viewport.width}px overflow: ${JSON.stringify(size)}`);
    assert.equal(await page.locator("#generate").isEnabled(), true);
    assert.equal(await page.locator(".katex-error").count(), 0);
  }
  await page.setViewportSize({ width: 1440, height: 1100 });
}

try {
  await page.goto(base);
  await page.waitForFunction(() => !document.getElementById("generate").disabled);
  assert.equal(await page.locator("#empty-state").isVisible(), true);
  const selectable = new Set();
  for (let level = 1; level <= 5; level++) {
    await page.locator(`[data-level="${level}"]`).click();
    for (const id of await page.locator("#family option").evaluateAll((options) => options.map((o) => o.value))) {
      if (id !== "mixed") selectable.add(id);
    }
  }
  assert.deepEqual([...selectable].sort(), ready.families.map((f) => f.id).sort());
  await page.locator('[data-level="1"]').click();
  const firstResponse = page.waitForResponse((r) => r.url().endsWith("/api/generate") && r.status() === 202);
  await page.locator("#generate").click();
  const firstJob = await (await firstResponse).json();
  const concurrent = await fetch(`${base}/api/generate`, {
    method: "POST", headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ level: 1, family: "mixed" }),
  });
  assert.equal(concurrent.status, 409);
  await waitGenerated();
  const firstId = await page.locator("#problem-label").textContent();
  assert.equal(await page.locator("#problem-family").isVisible(), false);
  await page.locator("#verification-panel summary").click();
  assert.equal(await page.locator("#verification-metadata [data-reveals-family]").first().isVisible(), false);
  await page.locator("#verification-panel summary").click();
  assert.match(await page.locator("#download-tex").getAttribute("href"), /solution=false$/);
  await page.locator("#hint-panel summary").click();
  assert.equal(await page.locator("#solution-panel").getAttribute("open"), null);
  assert.match(await page.locator("#download-tex").getAttribute("href"), /solution=false$/);
  await openAnswer();
  assert.equal(await page.locator("#problem-family").isVisible(), true);
  const initial = await jobResult(firstJob.id);
  const recipe = await (await fetch(`${base}/api/problems/${initial.problem.id}/recipe`)).json();
  const startCancel = page.waitForResponse((r) => r.url().endsWith("/api/generate") && r.status() === 202);
  await page.locator("#generate").click();
  await startCancel;
  await page.locator("#cancel").click();
  await page.locator("#job-progress").waitFor({ state: "hidden", timeout: 5000 });
  assert.equal(await page.locator("#problem-label").textContent(), firstId);
  assert.equal(await page.locator("#generate").isEnabled(), true);
  await page.locator("#dismiss-notice").click();
  await page.locator("#recipe-file").setInputFiles({
    name: "saved-recipe.json", mimeType: "application/json", buffer: Buffer.from(JSON.stringify(recipe)),
  });
  await waitGenerated();
  assert.equal(await page.locator("#problem-label").textContent(), firstId);

  const pair = await generateFamily(4, "coupled_scaled");
  assert.equal(pair.problem.problem.kind, "system");
  assert.equal(pair.problem.problem.secondary_recurrences.length, 1);
  assert.equal(pair.problem.problem.secondary_general_terms.length, 1);
  const statement = await annotation("#statement");
  assert.equal(statement, pair.problem.statement_latex);
  assert.match(statement, /a_\{n\+1\}/);
  assert.match(statement, /b_\{n\+1\}/);
  assert.match(statement, /a_\{1\}/);
  assert.match(statement, /b_\{1\}/);
  assert.equal(await page.locator(".answer-box").isVisible(), false);
  const recurrences = gatheredRows(pair.problem.statement_latex).slice(0, 2);
  const answers = gatheredRows(pair.problem.answer_latex);
  assert.equal(answers.length, 2);
  const questionTex = await downloadedTex();
  for (const equation of recurrences) assert.ok(questionTex.includes(equation), "TeX must contain both recurrences");
  for (const equation of answers) assert.ok(!questionTex.includes(equation), "Closed-answer TeX must hide both answers");
  assert.ok(!questionTex.includes("\\subsection*{解答}"));
  await page.locator("#hint-panel summary").click();
  assert.equal(await downloadedTex(), questionTex, "Opening a hint must keep the TeX answer hidden");
  await openAnswer();
  assert.equal(await annotation(".answer-box"), pair.problem.answer_latex);
  const fullTex = await downloadedTex();
  for (const equation of [...recurrences, ...answers]) assert.ok(fullTex.includes(equation), "Full TeX must contain both equations and both answers");
  assert.ok(fullTex.includes("\\subsection*{解答}"));
  await assertResponsive("paired recurrences and answers");

  for (const [level, family] of [[2, "pure_sum"], [5, "pure_sum_scaled"]]) {
    const sum = await generateFamily(level, family);
    const sumMath = await annotation("#statement");
    assert.equal(sumMath, sum.problem.statement_latex);
    assert.match(sumMath, /\\sum_\{k=1\}/);
    if (family === "pure_sum_scaled") assert.match(sumMath, /\^\{n\+2\}/);
    await openAnswer();
    assert.match(await page.locator("#solution-content").textContent(), /S₀=0/);
    assert.equal(await annotation(".answer-box"), sum.problem.answer_latex);
    await assertResponsive(family);
  }

  const power = await generateFamily(4, "ratio_power");
  assert.equal(power.problem.recipe.parameters.profile.num, "2");
  assert.match(await annotation("#statement"), /\\binom/);
  await openAnswer();
  assert.match(await annotation(".answer-box"), /\\binom/);
  await assertResponsive("binomial exponent");

  const logarithm = await generateFamily(5, "multiplicative_second");
  await openAnswer();
  const solutionMath = await page.locator('#solution-content annotation[encoding="application/x-tex"]').allTextContents();
  assert.ok(solutionMath.some((latex) => latex.includes("\\log_{")), "The actual logarithm substitution must render as mathematics");
  assert.equal(await annotation(".answer-box"), logarithm.problem.answer_latex);
  await assertResponsive("logarithm and double differences");

  await page.locator("#recipe-file").setInputFiles({
    name: "paired-preview.json", mimeType: "application/json", buffer: Buffer.from(JSON.stringify(pair.problem.recipe)),
  });
  await waitGenerated();
  assert.equal(await annotation("#statement"), pair.problem.statement_latex);
  await page.locator("#hint-panel summary").click();
  await page.screenshot({ path: "test-results/desktop.png", fullPage: true });
  await page.screenshot({ path: "docs/preview.png", fullPage: true });
  await page.setViewportSize({ width: 390, height: 844 });
  await page.screenshot({ path: "test-results/mobile.png", fullPage: true });
  await page.screenshot({ path: "docs/mobile-preview.png", fullPage: true });
  assert.deepEqual(errors, []);
  assert.deepEqual(consoleErrors, []);
  assert.deepEqual(external, []);
  console.log("PASS: 28-family catalog; generation/concurrency/cancellation/replay; paired equations, initials and answers; TeX answer privacy; sum boundaries; binomial/log rendering; offline assets; 1440px/390px layouts");
} finally {
  await browser.close();
}
