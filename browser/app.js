"use strict";
(() => {
  const $ = (id) => document.getElementById(id);
  const descriptions = [
    ["基礎をたしかめる", "等差・等比数列の基本形。"],
    ["変化の規則を見つける", "定数項の移動、階比、階乗、部分和を使う問題。"],
    ["数列を組み合わせる", "3項間、連立、指数の和を使って一般項へ。"],
    ["見方を変えて解く", "正規化、一次分数、特解や階差を組み合わせる問題。"],
    ["いくつもの発想をつなぐ", "総和・対数・階差など、複数の発想をつなぐ発展問題。"],
  ];
  let level = 1, families = [], ready = false, active = null, current = null, deploymentMode = null;
  let generation = 0, pollTimer = null, elapsedTimer = null, refreshTimer = null;
  let saveGeneration = 0, texUrl = null, recipeUrl = null;
  const terminal = new Set(["succeeded", "failed", "timeout", "cancelled"]);
  const browserRuntime = () => window.RecurrenceBrowser?.active?.() === true;
  async function api(path, options = {}) {
    if (browserRuntime()) return window.RecurrenceBrowser.request(path, options);
    const response = await fetch(path, { ...options, headers: { "Content-Type": "application/json", ...options.headers } });
    const result = await response.json();
    if (!response.ok) throw new Error(result.error || `通信に失敗しました (${response.status})`);
    return result;
  }
  function notice(message, kind = "error") {
    $("notice").hidden = false; $("notice").className = `notice ${kind}`; $("notice-text").textContent = message;
  }
  function metadata(target, entries) {
    const node = $(target); node.replaceChildren();
    entries.forEach(([key, value]) => {
      const dt = document.createElement("dt"), dd = document.createElement("dd");
      dt.textContent = key; dd.textContent = String(value ?? "—"); node.append(dt, dd);
      if (key === "分類の理由") { dt.dataset.revealsFamily = "true"; dd.dataset.revealsFamily = "true"; }
    });
  }
  function math(target, latex) {
    target.replaceChildren();
    try { window.katex.render(latex, target, { displayMode: true, throwOnError: true, trust: false }); }
    catch (e) { target.textContent = latex; console.error("数式の表示に失敗", e.message); }
  }
  function refreshFamilies() {
    const select = $("family"), old = select.value;
    select.replaceChildren(new Option("総合演習（おまかせ）", "mixed"));
    families.filter((f) => f.levels.includes(level)).forEach((f) => select.add(new Option(f.label, f.id)));
    if ([...select.options].some((o) => o.value === old)) select.value = old;
    $("family-help").textContent = select.value === "mixed" ? "解法の型は、解答を開くまでお楽しみに。" : "選んだ型に合う問題を、その場で生成します。";
  }
  function controls() {
    $("generate").disabled = !ready || !!active;
    $("family").disabled = !!active;
    document.querySelectorAll(".level-button").forEach((b) => { b.disabled = !!active; });
    $("import-recipe-global").disabled = !ready || !!active;
    $("load-recipe").disabled = !ready || !!active;
  }
  function deployment(mode) {
    deploymentMode = mode === "browser" ? "browser" : "local";
    const browser = deploymentMode === "browser";
    $("deployment-label").textContent = browser ? "このブラウザーで実行" : "ローカル実行";
    $("environment-title").textContent = browser ? "ブラウザーの準備" : "実行環境";
    $("deployment-footnote").replaceChildren();
    const lines = browser
      ? ["問題は、このブラウザー内で生成。", "HTMLを開くだけで演習を始められます。"]
      : ["生成も検証も、このPCで。", "初回準備後はオフラインで使えます。"];
    $("deployment-footnote").append(lines[0], document.createElement("br"), lines[1]);
    $("deployment-edition").textContent = `${browser ? "BROWSER" : "LOCAL"} EDITION / 03`;
    $("runtime-edition").textContent = browser ? "ブラウザー内で生成" : "Rust × Lean";
    $("generation-note-text").textContent = browser
      ? "有理数の計算で条件を確認した一問を出題します。"
      : "Leanで検証できた一問を出題します。";
    $("verification-explanation").textContent = browser
      ? "ブラウザー版は有理数による有限範囲の計算確認を行います。生成した問題に対してLeanは実行していません。難易度は学習の目安です。"
      : "検証は初期条件・漸化式・一般項・一意性を対象とします。難易度は学習の目安です。";
  }
  async function status() {
    try {
      const result = await api("/api/status");
      deployment(result.deployment_mode);
      const browser = deploymentMode === "browser";
      ready = result.ready; families = result.families || []; refreshFamilies();
      $("environment-badge").textContent = ready ? "準備完了" : "準備が必要";
      $("environment-badge").className = `environment-badge ${ready ? "ready" : "unready"}`;
      $("environment-message").textContent = browser
        ? (ready ? "問題の生成と計算の確認は、このブラウザー内で行います。" : result.verification?.message || "問題を生成する準備をしています。")
        : (ready ? "Leanと共通モジュールの準備が整っています。" : result.verification?.message);
      $("setup-help").hidden = ready || browser;
      $("setup-command").textContent = "bash scripts/setup-lean.sh";
      metadata("environment-metadata", browser
        ? [["実行", "このブラウザー内"], ["計算", "厳密な有理数"], ["確認範囲", "初期条件・漸化式（n=1〜20）"],
          ["Lean", "実行していません"], ["アプリ", result.application_version], ["対応する型", `${families.length} 系統`]]
        : [["Lean", result.verification?.version], ["アプリ", result.application_version],
          ["検証の上限", `${result.limits?.lean_timeout_seconds} 秒 / 1問`], ["対応する型", `${families.length} 系統`]]);
      if (result.active_job && !active) {
        active = { id: result.active_job, started: Date.now(), token: ++generation, mixed: true };
        beginProgress(); poll(active.token);
      }
      clearTimeout(refreshTimer); if (!ready) refreshTimer = setTimeout(status, 10000);
    } catch (e) {
      const browser = browserRuntime() || location.protocol === "file:";
      if (browser) deployment("browser");
      ready = false; $("environment-badge").textContent = browser ? "読み込み失敗" : "未接続";
      $("environment-badge").className = "environment-badge disconnected";
      $("environment-message").textContent = browser
        ? "ブラウザー版を読み込めませんでした。配布ファイルをすべて展開してからHTMLを開き直してください。"
        : "アプリに接続できません。通信状況を確認してください。";
      if (browser) $("setup-help").hidden = true;
      clearTimeout(refreshTimer); refreshTimer = setTimeout(status, 10000);
    }
    controls();
  }
  function beginProgress() {
    $("job-progress").hidden = false; $("cancel").disabled = !active?.id; $("cancel").textContent = "生成を中止する";
    $("job-phase").textContent = "候補を作成中"; $("job-message").textContent = "条件に合う問題を探しています。";
    clearInterval(elapsedTimer);
    elapsedTimer = setInterval(() => { if (active) $("elapsed").textContent = `${((Date.now() - active.started) / 1000).toFixed(1)} 秒`; }, 100);
    controls();
  }
  async function start(path, body, mixed) {
    if (!ready || active) return;
    const token = ++generation;
    active = { id: null, started: Date.now(), token, mixed };
    $("notice").hidden = true; beginProgress();
    try {
      const result = await api(path, { method: "POST", body: typeof body === "string" ? body : JSON.stringify(body) });
      if (!active || active.token !== token) return;
      active.id = result.id; $("cancel").disabled = false; poll(token);
    } catch (e) { endProgress(token); notice(e.message); }
  }
  async function poll(token) {
    if (!active || active.token !== token || !active.id) return;
    try {
      const job = await api(`/api/jobs/${encodeURIComponent(active.id)}`);
      if (!active || active.token !== token) return;
      $("elapsed").textContent = `${(job.elapsed_ms / 1000).toFixed(1)} 秒`;
      $("job-phase").textContent = job.phase === "cancelling" ? "処理を終了中" : ["verifying", "browser_check"].includes(job.status)
        ? (deploymentMode === "browser" ? "有理数で計算を確認中" : "Leanで検証中") : "候補を作成中";
      $("job-message").textContent = job.message;
      if (terminal.has(job.status)) {
        const mixed = active.mixed;
        const checked = deploymentMode === "browser" ? job.verification?.status === "browser_checked" : job.verification?.status === "success";
        if (job.status === "succeeded" && job.problem && checked) renderProblem(job, mixed);
        else if (job.status === "cancelled") notice("生成を中止しました。次の問題をつくれます。", "info");
        else notice(`${job.message} 出題設定を変更するか、もう一度お試しください。`);
        endProgress(token); return;
      }
      pollTimer = setTimeout(() => poll(token), 350);
    } catch (e) {
      if (!active || active.token !== token) return;
      notice(deploymentMode === "browser" ? "処理状況の確認を再試行しています。" : "処理状況に接続できません。接続が戻ると確認を再開します。", "info");
      pollTimer = setTimeout(() => poll(token), 1500);
    }
  }
  function endProgress(token) {
    if (!active || active.token !== token) return;
    clearTimeout(pollTimer); clearInterval(elapsedTimer); active = null;
    $("job-progress").hidden = true; controls();
  }
  async function updateSave() {
    if (!current) return;
    const problem = current.problem, token = ++saveGeneration;
    const show = $("solution-panel").open;
    $("save-description").textContent = show ? "問題・方針・解答を保存" : "問題のみを保存";
    $("download-tex").download = `recurrence-${problem.id.slice(0, 12)}.tex`;
    $("problem-family").hidden = current.mixed && !show;
    $("verification-metadata").querySelectorAll("[data-reveals-family]").forEach((node) => { node.hidden = current.mixed && !show; });
    if (texUrl) { URL.revokeObjectURL(texUrl); texUrl = null; }
    if (deploymentMode !== "browser") {
      $("download-tex").href = `/api/problems/${problem.id}/tex?solution=${show}`;
      $("download-tex").removeAttribute("aria-disabled"); return;
    }
    $("download-tex").removeAttribute("href"); $("download-tex").setAttribute("aria-disabled", "true");
    try {
      const source = await window.RecurrenceBrowser.tex(problem, show);
      if (token !== saveGeneration || current?.problem !== problem || $("solution-panel").open !== show) return;
      texUrl = URL.createObjectURL(new Blob([source], { type: "application/x-tex;charset=utf-8" }));
      $("download-tex").href = texUrl; $("download-tex").removeAttribute("aria-disabled");
    } catch (e) {
      if (token === saveGeneration && current?.problem === problem) notice(`TeXを書き出せませんでした。${e.message}`);
    }
  }
  function renderProblem(job, mixed) {
    current = { problem: job.problem, mixed }; const p = job.problem, v = job.verification;
    $("hint-panel").open = false; $("solution-panel").open = false; $("verification-panel").open = false;
    $("empty-state").hidden = true; $("problem-content").hidden = false; $("verification-panel").hidden = false;
    $("problem-level").textContent = `Lv. ${p.classification.level}`;
    $("problem-family").textContent = p.family_label; $("problem-label").textContent = `PROBLEM ${p.id.slice(0, 8).toUpperCase()}`;
    math($("statement"), p.statement_latex);
    $("problem-conditions").textContent = "数列の項は有理数とする。nは1以上の整数。";
    $("hint-content").textContent = p.hint;
    const container = $("solution-content"); container.replaceChildren();
    p.solution.forEach((step, i) => {
      const block = document.createElement("div"), paragraph = document.createElement("p");
      block.className = "solution-step"; paragraph.textContent = step.text; block.append(paragraph);
      if (step.latex) { const eq = document.createElement("div"); eq.className = "solution-equation"; math(eq, step.latex); block.append(eq); }
      container.append(block);
    });
    const answer = document.createElement("div"); answer.className = "answer-box";
    const label = document.createElement("strong"); label.textContent = "一般項";
    const answerMath = document.createElement("div"); math(answerMath, p.answer_latex); answer.append(label, answerMath); container.append(answer);
    const browser = deploymentMode === "browser";
    $("verification-label").textContent = browser ? `${v.checked_terms || 20}項 計算確認済み` : "Lean 検証済み";
    const verification = browser
      ? [["実行エンジン", v.engine || "Rust / WebAssembly"], ["計算", "厳密な有理数"],
        ["確認範囲", `n=${v.first_index || 1}〜${v.last_index || v.checked_terms || 20}`],
        ["確認項目", "初期条件・漸化式・非零条件"], ["Lean", "実行していません"],
        ["Recipeの再現", v.recipe_replayed ? "一致を確認" : "—"], [v.recipe_hash ? "Recipeハッシュ" : "問題ハッシュ", v.recipe_hash || v.problem_hash || p.id]]
      : [["Lean", v.lean_version], ["証明ハッシュ", v.proof_hash], ["公理監査", (v.axioms || []).join(", ") || "依存なし"]];
    metadata("verification-metadata", [["問題ID", p.id], ...verification, [browser ? "確認時間" : "検証時間", `${((v.elapsed_ms || 0) / 1000).toFixed(2)} 秒`],
      ["出題のスコア", `${p.classification.score} / Lv.${p.classification.level}`],
      ["分類の理由", p.classification.reason], ["操作 B / 発見 R / 計算 A / 条件 T / 式 P / 未評価 U", Object.values(p.classification.parts).join(" / ")],
      ["規則の版", p.recipe.rules_version]]);
    if (recipeUrl) { URL.revokeObjectURL(recipeUrl); recipeUrl = null; }
    if (browser) {
      recipeUrl = URL.createObjectURL(new Blob([JSON.stringify(p.recipe, null, 2)], { type: "application/json;charset=utf-8" }));
      $("download-recipe").href = recipeUrl;
    } else $("download-recipe").href = `/api/problems/${p.id}/recipe`;
    $("download-recipe").download = `recipe-${p.id.slice(0, 12)}.json`;
    updateSave();
  }
  document.querySelectorAll(".level-button").forEach((button) => button.addEventListener("click", () => {
    level = Number(button.dataset.level);
    document.querySelectorAll(".level-button").forEach((b) => { const selected = b === button; b.classList.toggle("selected", selected); b.setAttribute("aria-pressed", String(selected)); });
    $("level-title").textContent = descriptions[level - 1][0]; $("level-description").textContent = descriptions[level - 1][1];
    document.querySelectorAll(".difficulty-dots i").forEach((dot, i) => dot.classList.toggle("active", i < level)); refreshFamilies();
  }));
  $("family").addEventListener("change", refreshFamilies);
  $("generate").addEventListener("click", () => start("/api/generate", { level, family: $("family").value }, $("family").value === "mixed"));
  $("cancel").addEventListener("click", async () => {
    if (!active?.id) return;
    $("cancel").disabled = true; $("cancel").textContent = "中止しています…";
    try { await api(`/api/jobs/${active.id}/cancel`, { method: "POST" }); }
    catch (e) { notice(e.message); $("cancel").disabled = false; }
  });
  $("solution-panel").addEventListener("toggle", updateSave);
  $("download-tex").addEventListener("click", (event) => { if ($("download-tex").getAttribute("aria-disabled") === "true") event.preventDefault(); });
  $("dismiss-notice").addEventListener("click", () => { $("notice").hidden = true; });
  $("refresh-status").addEventListener("click", status);
  ["load-recipe", "import-recipe-global"].forEach((id) => $(id).addEventListener("click", () => $("recipe-file").click()));
  $("recipe-file").addEventListener("change", async (e) => {
    const file = e.target.files[0]; if (!file) return;
    try {
      if (file.size > 256 * 1024) throw new Error("Recipeファイルは256KB以下にしてください。");
      const source = await file.text(); JSON.parse(source);
      start("/api/replay", deploymentMode === "browser" ? { recipe: source } : `{"recipe":${source}}`, true);
    }
    catch (error) { notice(`Recipeを読み込めませんでした。${error.message}`); } e.target.value = "";
  });
  window.addEventListener("pagehide", () => {
    if (texUrl) URL.revokeObjectURL(texUrl);
    if (recipeUrl) URL.revokeObjectURL(recipeUrl);
  });
  if (browserRuntime()) deployment("browser");
  status();
})();
