"use strict";
(() => {
  const TIMEOUT_MS = 10000;
  const HISTORY_LIMIT = 40;
  const HISTORY_KEY = "recurrence-lab:browser:recent:v1";
  const terminal = new Set(["succeeded", "failed", "timeout", "cancelled"]);
  const jobs = new Map();
  const problems = new Map();
  const operations = new Set();
  let activeJob = null;
  let catalogPromise = null;
  let recent = [];
  let historyStorage = "memory";

  try {
    const saved = JSON.parse(window.localStorage.getItem(HISTORY_KEY) || "[]");
    if (Array.isArray(saved)) {
      recent = [...new Set(saved.filter((id) => typeof id === "string" && /^[a-f0-9]{64}$/.test(id)))].slice(-HISTORY_LIMIT);
    }
    historyStorage = "local";
  } catch (_) { /* Some browsers disable storage for file URLs. */ }

  function active() {
    return typeof window.RECURRENCE_ENGINE_SOURCE === "string";
  }
  function availability() {
    if (!active()) return "ブラウザー用の生成エンジンが見つかりません。配布したフォルダーを展開してindex.htmlを開いてください。";
    if (typeof WebAssembly !== "object") return "このブラウザーはWebAssemblyに対応していません。最新版のChrome、Edge、FirefoxまたはSafariで開いてください。";
    if (typeof Worker !== "function" || typeof Blob !== "function" || typeof URL.createObjectURL !== "function") {
      return "このブラウザーではバックグラウンド処理を利用できません。最新版のChrome、Edge、FirefoxまたはSafariで開いてください。";
    }
    return null;
  }
  function clone(value) {
    return value == null ? value : JSON.parse(JSON.stringify(value));
  }
  function id() {
    if (typeof window.crypto?.randomUUID === "function") return window.crypto.randomUUID();
    const bytes = new Uint8Array(16);
    if (typeof window.crypto?.getRandomValues === "function") window.crypto.getRandomValues(bytes);
    else for (let i = 0; i < bytes.length; i++) bytes[i] = Math.floor(Math.random() * 256);
    bytes[6] = (bytes[6] & 15) | 64;
    bytes[8] = (bytes[8] & 63) | 128;
    const hex = [...bytes].map((b) => b.toString(16).padStart(2, "0")).join("");
    return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
  }
  function seed() {
    if (typeof window.crypto?.getRandomValues === "function") {
      const words = new Uint32Array(2);
      window.crypto.getRandomValues(words);
      if (typeof BigInt === "function") return ((BigInt(words[0]) << 32n) | BigInt(words[1])).toString();
      return (words[0] * 2097152 + (words[1] >>> 11)).toString();
    }
    // This fallback diversifies exercises; it is not used for cryptographic purposes.
    return `${Date.now()}${Math.floor(Math.random() * 1000000).toString().padStart(6, "0")}`;
  }
  function operationError(message, code) {
    const error = new Error(message);
    error.code = code;
    return error;
  }
  function run(operation, payload) {
    const unavailable = availability();
    if (unavailable) throw new Error(unavailable);
    const operationId = id();
    const url = URL.createObjectURL(new Blob([window.RECURRENCE_ENGINE_SOURCE], { type: "text/javascript" }));
    let worker;
    try { worker = new Worker(url); }
    catch (_) {
      URL.revokeObjectURL(url);
      throw new Error("ブラウザーが生成エンジンの起動を制限しています。配布フォルダーを展開し、別の対応ブラウザーでindex.htmlを開いてください。");
    }
    let settled = false, timer = null, rejectPromise;
    const handle = { cancel: null, promise: null };
    function cleanup() {
      clearTimeout(timer);
      worker.terminate();
      URL.revokeObjectURL(url);
      operations.delete(handle);
    }
    function reject(error) {
      if (settled) return;
      settled = true; cleanup(); rejectPromise(error);
    }
    handle.promise = new Promise((resolve, rejectResult) => {
      rejectPromise = rejectResult;
      worker.onmessage = (event) => {
        const response = event.data;
        if (!response || response.id !== operationId || settled) return;
        if (response.error != null) {
          reject(new Error(String(response.error)));
          return;
        }
        try {
          const result = operation === "tex" ? String(response.result) : JSON.parse(response.result);
          settled = true; cleanup(); resolve(result);
        } catch (_) { reject(new Error("生成エンジンの応答を読み込めませんでした。ページを開き直してお試しください。")); }
      };
      worker.onerror = (event) => {
        if (typeof event.preventDefault === "function") event.preventDefault();
        reject(new Error("生成エンジンを実行できませんでした。配布フォルダーを展開し、対応ブラウザーで開き直してください。"));
      };
      worker.onmessageerror = () => reject(new Error("生成エンジンとのデータ交換に失敗しました。もう一度お試しください。"));
      timer = setTimeout(() => reject(operationError("処理が10秒を超えたため終了しました。別の設定でもう一度お試しください。", "timeout")), TIMEOUT_MS);
      try { worker.postMessage({ id: operationId, operation, payload }); }
      catch (_) { reject(new Error("生成エンジンへデータを渡せませんでした。入力ファイルを確認してください。")); }
    });
    handle.cancel = () => reject(operationError("生成を中止しました。", "cancelled"));
    if (!settled) operations.add(handle);
    return handle;
  }
  async function catalog() {
    if (!catalogPromise) {
      catalogPromise = run("catalog", null).promise.then((result) => {
        if (!result || result.ready !== true || !Array.isArray(result.families)) {
          throw new Error("生成エンジンの設定を読み込めませんでした。配布フォルダーを確認してください。");
        }
        return result;
      }).catch((error) => {
        catalogPromise = null;
        throw error;
      });
    }
    return catalogPromise;
  }
  function remember(problem) {
    recent = [...recent.filter((value) => value !== problem.id), problem.id].slice(-HISTORY_LIMIT);
    try {
      window.localStorage.setItem(HISTORY_KEY, JSON.stringify(recent));
      historyStorage = "local";
    } catch (_) { historyStorage = "memory"; }
    problems.delete(problem.id);
    problems.set(problem.id, clone(problem));
    while (problems.size > HISTORY_LIMIT) problems.delete(problems.keys().next().value);
  }
  function snapshot(job) {
    return clone({
      id: job.id, status: job.status, phase: job.phase, message: job.message,
      elapsed_ms: job.elapsed_ms ?? Math.max(0, Date.now() - job.started),
      problem: job.problem || null, verification: job.verification || null,
    });
  }
  function finish(job, status, message) {
    job.status = status; job.phase = status; job.message = message;
    job.elapsed_ms = Math.max(0, Date.now() - job.started);
    job.operation = null;
    if (activeJob === job.id) activeJob = null;
  }
  async function execute(job, operation, payload) {
    try {
      job.operation = run(operation, payload);
      job.status = "browser_check"; job.phase = "browser_check";
      job.message = "ブラウザー内でRecipeと最初の20項を確認しています。";
      const problem = await job.operation.promise;
      if (terminal.has(job.status)) return;
      const checked = problem?.browser_check;
      if (!problem?.id || checked?.status !== "browser_checked" || checked.formal_verification !== false) {
        throw new Error("ブラウザー確認の結果が正しくありません。ページを開き直してください。");
      }
      remember(problem);
      job.problem = clone(problem);
      job.verification = {
        ...clone(checked), status: "browser_checked", engine: "Rust / WebAssembly",
        elapsed_ms: Math.max(0, Date.now() - job.started), lean_executed: false,
        formal_verification: false, problem_hash: problem.id,
      };
      finish(job, "succeeded", "問題を生成しました。Recipeと最初の20項を確認済みです。Leanによる形式証明は実行していません。");
    } catch (error) {
      if (terminal.has(job.status)) return;
      finish(job, error.code === "timeout" ? "timeout" : error.code === "cancelled" ? "cancelled" : "failed", error.message || String(error));
    }
  }
  async function start(operation, payload) {
    await catalog();
    if (activeJob) throw new Error("生成が進行中です。終了するか中止してから次の問題をつくってください。");
    if (operation === "generate") {
      if (!Number.isInteger(payload.level) || payload.level < 1 || payload.level > 5) throw new Error("難易度はLv1〜Lv5を指定してください。");
      payload = { ...payload, seed: payload.seed ?? seed(), recent_ids: [...recent] };
    }
    const job = {
      id: id(), status: "generating", phase: "generating", message: "条件に合う問題を作成しています。",
      started: Date.now(), elapsed_ms: null, problem: null, verification: null, operation: null,
    };
    jobs.set(job.id, job); activeJob = job.id;
    for (const [oldId, oldJob] of jobs) {
      if (jobs.size <= HISTORY_LIMIT) break;
      if (terminal.has(oldJob.status)) jobs.delete(oldId);
    }
    // Yield once so the caller can receive a job ID before processing begins.
    Promise.resolve().then(() => { if (!terminal.has(job.status)) execute(job, operation, payload); });
    return { id: job.id, status: "generating" };
  }
  function body(options) {
    let result;
    if (typeof options.body === "string") {
      if (options.body.length > 256 * 1024) throw new Error("入力ファイルは256KB以下にしてください。");
      try { result = JSON.parse(options.body); }
      catch (_) { throw new Error("JSONの形式が正しくありません。Recipeファイルを確認してください。"); }
    } else result = clone(options.body || {});
    if (!result || typeof result !== "object" || Array.isArray(result)) throw new Error("入力はJSONオブジェクトで指定してください。");
    return result;
  }
  async function status() {
    let result, message = availability();
    if (!message) {
      try { result = await catalog(); }
      catch (error) { message = error.message || String(error); }
    }
    return {
      ready: !message && result?.ready !== false,
      deployment_mode: "browser", application_version: result?.application_version || "0.3.0",
      engine: "Rust / WebAssembly", families: result?.families || [],
      active_job: activeJob, history_count: recent.length, history_storage: historyStorage,
      checked_terms: result?.checked_terms || 20,
      limits: {
        candidate_limit: 200, search_timeout_seconds: 10, request_timeout_seconds: 10,
        browser_timeout_seconds: 10, lean_timeout_seconds: 0, lean_candidate_limit: 0,
        ...result?.limits,
      },
      verification: {
        ready: !message && result?.ready !== false, version: "Rust / WebAssembly",
        message: message || "生成エンジンをブラウザー内で利用できます。Recipeと最初の20項を確認します。Leanの形式証明は実行しません。",
        engine: "Rust / WebAssembly", checked_terms: result?.checked_terms || 20,
        lean_executed: false, formal_verification: false,
      },
    };
  }
  async function tex(problem, solution = false) {
    return run("tex", { problem: clone(problem), solution: Boolean(solution) }).promise;
  }
  async function request(path, options = {}) {
    if (!active()) throw new Error("ブラウザー用の生成エンジンが見つかりません。");
    const method = String(options.method || "GET").toUpperCase();
    const [pathname, query = ""] = String(path).split("?");
    if (pathname === "/api/status" && method === "GET") return status();
    if (pathname === "/api/generate" && method === "POST") return start("generate", body(options));
    if (pathname === "/api/replay" && method === "POST") {
      const input = body(options);
      if (!input.recipe || (typeof input.recipe !== "string" && typeof input.recipe !== "object") || Array.isArray(input.recipe)) {
        throw new Error("Recipeの形式が正しくありません。");
      }
      return start("replay", input.recipe);
    }
    const jobMatch = pathname.match(/^\/api\/jobs\/([^/]+)(\/cancel)?$/);
    if (jobMatch) {
      const job = jobs.get(decodeURIComponent(jobMatch[1]));
      if (!job) throw new Error("要求が見つかりません。このページで生成した問題だけを確認できます。");
      if (!jobMatch[2] && method === "GET") return snapshot(job);
      if (jobMatch[2] && method === "POST") {
        if (!terminal.has(job.status)) {
          const operation = job.operation;
          finish(job, "cancelled", "生成を中止しました。次の問題をつくれます。");
          operation?.cancel();
        }
        return { id: job.id, status: job.status, phase: job.phase };
      }
    }
    const problemMatch = pathname.match(/^\/api\/problems\/([^/]+)\/(recipe|tex)$/);
    if (problemMatch && method === "GET") {
      const problem = problems.get(decodeURIComponent(problemMatch[1]));
      if (!problem) throw new Error("問題が見つかりません。保存したRecipeを読み込んで再現してください。");
      if (problemMatch[2] === "recipe") return clone(problem.recipe);
      return tex(problem, new URLSearchParams(query).get("solution") === "true");
    }
    throw new Error("ブラウザー内で利用できない要求です。");
  }
  if (typeof window.addEventListener === "function") {
    window.addEventListener("pagehide", () => {
      for (const handle of [...operations]) handle.cancel();
    });
  }
  window.RecurrenceBrowser = Object.freeze({ active, request, tex });
})();
