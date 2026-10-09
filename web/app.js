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
  let level = 1, families = [], ready = false, active = null, current = null;
  let generation = 0, pollTimer = null, elapsedTimer = null, refreshTimer = null;
  const terminal = new Set(["succeeded", "failed", "timeout", "cancelled"]);
  async function api(path, options = {}) {
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
  async function status() {
    try {
      const result = await api("/api/status");
      ready = result.ready; families = result.families || []; refreshFamilies();
      $("environment-badge").textContent = ready ? "準備完了" : "準備が必要";
      $("environment-badge").className = `environment-badge ${ready ? "ready" : "checking"}`;
      $("environment-message").textContent = ready ? "Leanと共通モジュールの準備が整っています。" : result.verification.message;
      $("setup-help").hidden = ready;
      $("setup-command").textContent = "bash scripts/setup-lean.sh";
      metadata("environment-metadata", [["Lean", result.verification.version], ["アプリ", result.application_version],
        ["検証の上限", `${result.limits.lean_timeout_seconds} 秒 / 1問`], ["対応する型", `${families.length} 系統`]]);
      if (result.active_job && !active) {
        active = { id: result.active_job, started: Date.now(), token: ++generation, mixed: true };
        beginProgress(); poll(active.token);
      }
      clearTimeout(refreshTimer); if (!ready) refreshTimer = setTimeout(status, 10000);
    } catch (e) {
      ready = false; $("environment-badge").textContent = "未接続";
      $("environment-message").textContent = "ローカルアプリに接続できません。起動状況を確認してください。";
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
      const result = await api(path, { method: "POST", body: JSON.stringify(body) });
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
      $("job-phase").textContent = job.phase === "cancelling" ? "処理を終了中" : job.status === "verifying" ? "Leanで検証中" : "候補を作成中";
      $("job-message").textContent = job.message;
      if (terminal.has(job.status)) {
        const mixed = active.mixed;
        if (job.status === "succeeded" && job.problem && job.verification?.status === "success") renderProblem(job, mixed);
        else if (job.status === "cancelled") notice("生成を中止しました。次の問題をつくれます。", "info");
        else notice(`${job.message} 出題設定を変更するか、もう一度お試しください。`);
        endProgress(token); return;
      }
      pollTimer = setTimeout(() => poll(token), 350);
    } catch (e) {
      if (!active || active.token !== token) return;
      notice("処理状況に接続できません。接続が戻ると確認を再開します。", "info");
      pollTimer = setTimeout(() => poll(token), 1500);
    }
  }
  function endProgress(token) {
    if (!active || active.token !== token) return;
    clearTimeout(pollTimer); clearInterval(elapsedTimer); active = null;
    $("job-progress").hidden = true; controls();
  }
  function updateSave() {
    if (!current) return;
    const show = $("solution-panel").open;
    $("save-description").textContent = show ? "問題・方針・解答を保存" : "問題のみを保存";
    $("download-tex").href = `/api/problems/${current.problem.id}/tex?solution=${show}`;
    $("download-tex").download = `recurrence-${current.problem.id.slice(0, 12)}.tex`;
    $("problem-family").hidden = current.mixed && !show;
    $("verification-metadata").querySelectorAll("[data-reveals-family]").forEach((node) => { node.hidden = current.mixed && !show; });
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
    metadata("verification-metadata", [["問題ID", p.id], ["Lean", v.lean_version], ["検証時間", `${(v.elapsed_ms / 1000).toFixed(2)} 秒`],
      ["証明ハッシュ", v.proof_hash], ["出題のスコア", `${p.classification.score} / Lv.${p.classification.level}`],
      ["分類の理由", p.classification.reason], ["操作 B / 発見 R / 計算 A / 条件 T / 式 P / 未評価 U", Object.values(p.classification.parts).join(" / ")],
      ["公理監査", (v.axioms || []).join(", ") || "依存なし"], ["規則の版", p.recipe.rules_version]]);
    $("download-recipe").href = `/api/problems/${p.id}/recipe`; $("download-recipe").download = `recipe-${p.id.slice(0, 12)}.json`;
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
  $("dismiss-notice").addEventListener("click", () => { $("notice").hidden = true; });
  $("refresh-status").addEventListener("click", status);
  ["load-recipe", "import-recipe-global"].forEach((id) => $(id).addEventListener("click", () => $("recipe-file").click()));
  $("recipe-file").addEventListener("change", async (e) => {
    const file = e.target.files[0]; if (!file) return;
    try { if (file.size > 256 * 1024) throw new Error("Recipeファイルは256KB以下にしてください。"); const recipe = JSON.parse(await file.text()); start("/api/replay", { recipe }, true); }
    catch (error) { notice(`Recipeを読み込めませんでした。${error.message}`); } e.target.value = "";
  });
  status();
})();
