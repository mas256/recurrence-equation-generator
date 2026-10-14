const ORIGIN = 'https://mas256.github.io';
const REPO = 'mas256/math-textbooks';
const WORKFLOW = 'pages.yml';
const API = `https://api.github.com/repos/${REPO}/actions/workflows/${WORKFLOW}`;
const ACTIONS_URL = `https://github.com/${REPO}/actions/workflows/${WORKFLOW}`;
const COOLDOWN = 60 * 60 * 1000;

function json(data, status = 200, headers = {}) {
  return Response.json(data, { status, headers: { 'Cache-Control': 'no-store', ...headers } });
}

function publicRun(run) {
  if (!run) return null;
  return { id: run.id, status: run.status, conclusion: run.conclusion,
    url: `https://github.com/${REPO}/actions/runs/${run.id}` };
}

export default {
  async fetch(request, env) {
    const path = new URL(request.url).pathname;
    if (!['/regenerate', '/status'].includes(path)) return json({ error: 'not_found' }, 404);
    if (request.headers.get('Origin') !== ORIGIN) return json({ error: 'forbidden_origin' }, 403);
    const cors = { 'Access-Control-Allow-Origin': ORIGIN, Vary: 'Origin',
      'Access-Control-Allow-Methods': 'GET, POST, OPTIONS', 'Access-Control-Allow-Headers': 'Content-Type' };
    if (request.method === 'OPTIONS') return new Response(null, { status: 204, headers: cors });
    if ((path === '/regenerate' && request.method !== 'POST') ||
        (path === '/status' && request.method !== 'GET')) return json({ error: 'method_not_allowed' }, 405, cors);
    // This intentionally anonymous endpoint can only dispatch the fixed workflow on main.
    // Browser-origin checks are not authentication; the durable cooldown is the execution cap.
    if (request.method === 'POST' &&
        (request.body !== null || request.headers.has('Content-Length') && request.headers.get('Content-Length') !== '0')) {
      return json({ error: 'body_not_allowed' }, 400, cors);
    }
    if (!env.GITHUB_TOKEN || !env.REGENERATION_GATE) return json({ error: 'not_configured' }, 503, cors);
    try {
      const gate = env.REGENERATION_GATE.get(env.REGENERATION_GATE.idFromName('public-recurrence-bank'));
      const result = await gate.fetch(request);
      const response = new Response(result.body, result);
      for (const [key, value] of Object.entries(cors)) response.headers.set(key, value);
      return response;
    } catch {
      return json({ error: 'temporarily_unavailable' }, 503, cors);
    }
  },
};

export class RegenerationGate {
  constructor(ctx, env) {
    this.ctx = ctx;
    this.env = env;
    this.runsCache = null;
  }

  async github(url, options = {}) {
    return fetch(url, { ...options, signal: AbortSignal.timeout(10000), headers: {
      Accept: 'application/vnd.github+json', Authorization: `Bearer ${this.env.GITHUB_TOKEN}`,
      'X-GitHub-Api-Version': '2022-11-28', 'User-Agent': 'recurrence-regeneration-api',
      ...(options.headers || {}),
    } });
  }

  async runs() {
    if (this.runsCache && Date.now() - this.runsCache.time < 15000) return this.runsCache.value;
    if (!this.runsPending) {
      this.runsPending = (async () => {
        const response = await this.github(`${API}/runs?branch=main&per_page=20`);
        if (!response.ok) throw new Error('GitHub runs unavailable');
        const value = (await response.json()).workflow_runs;
        if (!Array.isArray(value)) throw new Error('Invalid GitHub response');
        this.runsCache = { time: Date.now(), value };
        return value;
      })().finally(() => { this.runsPending = null; });
    }
    return this.runsPending;
  }

  async fetch(request) {
    if (request.method === 'GET') {
      const [record, runs] = await Promise.all([this.ctx.storage.get('request'), this.runs()]);
      let requested = record && runs.find(run => record.activeRunId
        ? run.id === record.activeRunId : run.display_title === `Recurrence regeneration ${record.id}`);
      if (!requested && record?.activeRunId) {
        const response = await this.github(`https://api.github.com/repos/${REPO}/actions/runs/${record.activeRunId}`);
        if (!response.ok) throw new Error('GitHub run unavailable');
        requested = await response.json();
      }
      if (requested && !record.activeRunId) {
        await this.ctx.blockConcurrencyWhile(async () => {
          const current = await this.ctx.storage.get('request');
          if (current?.id === record.id && current.nextAllowedAt === record.nextAllowedAt) {
            await this.ctx.storage.put('request', { ...current, activeRunId: requested.id });
          }
        });
      }
      const run = record ? requested : runs[0];
      return json({ actionsUrl: ACTIONS_URL, requestId: record?.id || null,
        nextAllowedAt: record?.nextAllowedAt || 0,
        pending: Boolean(record && !requested && record.nextAllowedAt > Date.now()), run: publicRun(run) });
    }
    // Persist the reservation before any network call, so parallel requests and
    // isolate restarts cannot issue duplicate dispatches.
    const reservation = await this.ctx.blockConcurrencyWhile(async () => {
      const previous = await this.ctx.storage.get('request');
      const now = Date.now();
      if (previous && previous.nextAllowedAt > now) return { previous };
      const record = { id: crypto.randomUUID(), nextAllowedAt: now + COOLDOWN };
      await this.ctx.storage.put('request', record);
      return { record };
    });
    if (reservation.previous) {
      return json({ error: 'cooldown', nextAllowedAt: reservation.previous.nextAllowedAt, actionsUrl: ACTIONS_URL },
        429, { 'Retry-After': String(Math.max(1, Math.ceil((reservation.previous.nextAllowedAt - Date.now()) / 1000))) });
    }
    try {
      const runs = await this.runs();
      const active = runs.find(run => run.status !== 'completed');
      if (active) {
        // An existing deployment already rebuilds the bank. Track it instead of enqueueing another.
        await this.ctx.storage.put('request', { ...reservation.record, id: null, activeRunId: active.id });
        return json({ accepted: false, run: publicRun(active), nextAllowedAt: reservation.record.nextAllowedAt, actionsUrl: ACTIONS_URL }, 202);
      }
      const response = await this.github(`${API}/dispatches`, { method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ ref: 'main', inputs: { regeneration_request: reservation.record.id } }) });
      if (!response.ok) {
        if (response.status >= 400 && response.status < 500) await this.ctx.storage.delete('request');
        return json({ error: 'dispatch_failed', actionsUrl: ACTIONS_URL }, 502);
      }
      this.runsCache = null;
      return json({ accepted: true, requestId: reservation.record.id,
        nextAllowedAt: reservation.record.nextAllowedAt, actionsUrl: ACTIONS_URL }, 202);
    } catch {
      // Keep the cooldown after ambiguous network failures: GitHub may have accepted the request.
      return json({ error: 'temporarily_unavailable', actionsUrl: ACTIONS_URL }, 503);
    }
  }
}
