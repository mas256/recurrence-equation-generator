import test from 'node:test';
import assert from 'node:assert/strict';
import worker, { RegenerationGate } from './worker.mjs';

const origin = 'https://mas256.github.io';
const request = (path, method = 'GET', headers = {}, body) => new Request('https://example.workers.dev' + path,
  { method, headers: { Origin: origin, ...headers }, ...(body === undefined ? {} : { body }) });

function context(values = new Map()) {
  let lock = Promise.resolve();
  return { storage: { get: async key => values.get(key),
    put: async (key, value) => values.set(key, value), delete: async key => values.delete(key) },
  blockConcurrencyWhile(fn) { const result = lock.then(fn); lock = result.catch(() => {}); return result; } };
}

function setup(mock) {
  const calls = [];
  const previous = globalThis.fetch;
  globalThis.fetch = async (url, options) => {
    calls.push({ url, options });
    return mock(url, options);
  };
  const values = new Map();
  const ctx = context(values);
  const env = { GITHUB_TOKEN: 'unit-test-token' };
  const gate = new RegenerationGate(ctx, env);
  env.REGENERATION_GATE = { idFromName: name => name, get: () => gate };
  return { gate, env, values, calls, restore: () => { globalThis.fetch = previous; } };
}

test('origin, methods and request bodies cannot select arbitrary dispatch inputs', async () => {
  assert.equal((await worker.fetch(request('/regenerate', 'POST', { Origin: 'https://evil.example' }), {})).status, 403);
  assert.equal((await worker.fetch(request('/status', 'POST'), {})).status, 405);
  assert.equal((await worker.fetch(request('/regenerate', 'POST', {}, '{"ref":"evil"}'), {})).status, 400);
  const options = await worker.fetch(request('/regenerate', 'OPTIONS'), {});
  assert.equal(options.status, 204);
  assert.equal(options.headers.get('Access-Control-Allow-Origin'), origin);
  assert.equal((await worker.fetch(request('/status'), {})).status, 503);
  const emptyStream = new Request('https://example.workers.dev/regenerate', {
    method: 'POST', headers: { Origin: origin }, duplex: 'half',
    body: new ReadableStream({ start(controller) { controller.close(); } }),
  });
  assert.equal((await worker.fetch(emptyStream, {})).status, 503);
  const validEmptyStream = new Request('https://example.workers.dev/regenerate', {
    method: 'POST', headers: { Origin: origin }, duplex: 'half',
    body: new ReadableStream({ start(controller) { controller.close(); } }),
  });
  const env = { GITHUB_TOKEN: 'unit-test-token', REGENERATION_GATE: {
    idFromName: name => name, get: () => ({ fetch(internal) {
      assert.equal(internal.method, 'POST');
      assert.equal(internal.body, null);
      return Response.json({ accepted: true }, { status: 202 });
    } }),
  } };
  assert.equal((await worker.fetch(validEmptyStream, env)).status, 202);
});

test('parallel clicks and a restarted isolate issue only one dispatch', async () => {
  const s = setup((url) => url.includes('/dispatches') ? new Response(null, { status: 204 }) : Response.json({ workflow_runs: [] }));
  try {
    const results = await Promise.all(Array.from({ length: 8 }, () => worker.fetch(request('/regenerate', 'POST'), s.env)));
    assert.equal(results.filter(r => r.status === 202).length, 1);
    assert.equal(results.filter(r => r.status === 429).length, 7);
    const dispatches = s.calls.filter(call => call.url.endsWith('/dispatches'));
    assert.equal(dispatches.length, 1);
    const payload = JSON.parse(dispatches[0].options.body);
    assert.equal(payload.ref, 'main');
    assert.match(payload.inputs.regeneration_request, /^[0-9a-f-]{36}$/);
    assert.equal(dispatches[0].options.headers.Authorization, 'Bearer unit-test-token');
    assert.ok(dispatches[0].url.includes('/mas256/math-textbooks/actions/workflows/pages.yml/'));
    const restarted = new RegenerationGate(context(s.values), s.env);
    assert.equal((await restarted.fetch(request('/regenerate', 'POST'))).status, 429);
    assert.equal(results[0].headers.get('Access-Control-Allow-Origin'), origin);
  } finally { s.restore(); }
});

test('request status follows its run instead of reporting an older successful deployment', async () => {
  const s = setup(() => Response.json({ workflow_runs: [{ id: 1, display_title: 'Deploy Pages', status: 'completed', conclusion: 'success' }] }));
  try {
    s.values.set('request', { id: 'new-request', nextAllowedAt: Date.now() + 60000 });
    const pending = await (await s.gate.fetch(request('/status'))).json();
    assert.equal(pending.pending, true);
    assert.equal(pending.run, null);
    s.gate.runsCache = null;
    globalThis.fetch = async () => Response.json({ workflow_runs: [{ id: 2,
      display_title: 'Recurrence regeneration new-request', status: 'completed', conclusion: 'failure' }] });
    const finished = await (await s.gate.fetch(request('/status'))).json();
    assert.equal(finished.pending, false);
    assert.equal(finished.run.conclusion, 'failure');
    assert.equal(s.values.get('request').activeRunId, 2);
    // Follow a persisted run ID even after it falls outside the recent-runs page.
    s.gate.runsCache = null;
    globalThis.fetch = async url => url.includes('/actions/runs/2')
      ? Response.json({ id: 2, status: 'completed', conclusion: 'failure' }) : Response.json({ workflow_runs: [] });
    assert.equal((await (await s.gate.fetch(request('/status'))).json()).run.id, 2);
  } finally { s.restore(); }
});

test('an already active deployment is reused and remains trackable', async () => {
  const s = setup(() => Response.json({ workflow_runs: [{ id: 99, status: 'in_progress', conclusion: null }] }));
  try {
    const result = await (await s.gate.fetch(request('/regenerate', 'POST'))).json();
    assert.equal(result.accepted, false);
    assert.equal(result.run.id, 99);
    assert.ok(!s.calls.some(call => call.url.endsWith('/dispatches')));
    const status = await (await s.gate.fetch(request('/status'))).json();
    assert.equal(status.pending, false);
    assert.equal(status.run.id, 99);
  } finally { s.restore(); }
});

test('ambiguous failures keep the reservation and do not expose upstream secrets', async () => {
  const s = setup(url => {
    if (url.endsWith('/dispatches')) throw new Error('unit-test-token');
    return Response.json({ workflow_runs: [] });
  });
  try {
    const result = await worker.fetch(request('/regenerate', 'POST'), s.env);
    assert.equal(result.status, 503);
    assert.ok(!(await result.text()).includes('unit-test-token'));
    assert.equal((await worker.fetch(request('/regenerate', 'POST'), s.env)).status, 429);
  } finally { s.restore(); }
});

test('a definite GitHub rejection releases the reservation for configuration repairs', async () => {
  const s = setup(url => url.endsWith('/dispatches') ? new Response('private error details', { status: 403 }) : Response.json({ workflow_runs: [] }));
  try {
    const result = await worker.fetch(request('/regenerate', 'POST'), s.env);
    assert.equal(result.status, 502);
    assert.ok(!(await result.text()).includes('private error details'));
    assert.equal(s.values.has('request'), false);
  } finally { s.restore(); }
});

test('polls share one cached GitHub read across all visitors', async () => {
  const s = setup(() => Response.json({ workflow_runs: [] }));
  try {
    await Promise.all(Array.from({ length: 8 }, () => s.gate.fetch(request('/status'))));
    await s.gate.fetch(request('/status'));
    assert.equal(s.calls.length, 1);
  } finally { s.restore(); }
});

test('a late status response cannot overwrite a newer cooldown reservation', async () => {
  let resolveRuns;
  const delayed = new Promise(resolve => { resolveRuns = resolve; });
  const s = setup(url => url.endsWith('/dispatches') ? new Response(null, { status: 204 }) : delayed);
  try {
    s.values.set('request', { id: 'old-request', nextAllowedAt: Date.now() - 1 });
    const oldStatus = s.gate.fetch(request('/status'));
    await new Promise(resolve => setImmediate(resolve));
    const newRequest = s.gate.fetch(request('/regenerate', 'POST'));
    await new Promise(resolve => setImmediate(resolve));
    const newId = s.values.get('request').id;
    assert.notEqual(newId, 'old-request');
    resolveRuns(Response.json({ workflow_runs: [{ id: 77, display_title: 'Recurrence regeneration old-request', status: 'completed', conclusion: 'success' }] }));
    await Promise.all([oldStatus, newRequest]);
    assert.equal(s.values.get('request').id, newId);
    assert.equal((await s.gate.fetch(request('/regenerate', 'POST'))).status, 429);
  } finally { s.restore(); }
});

test('a dispatch with no observable run can be retried after the cooldown', async () => {
  const s = setup(() => Response.json({ workflow_runs: [] }));
  try {
    s.values.set('request', { id: 'uncertain-request', nextAllowedAt: Date.now() - 1 });
    const result = await (await s.gate.fetch(request('/status'))).json();
    assert.equal(result.pending, false);
  } finally { s.restore(); }
});
