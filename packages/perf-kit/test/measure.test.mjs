import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import { findChrome, launchBrowser } from '../lib/cdp.mjs';
import {
  DEFAULT_THROTTLE,
  networkThrottle,
  parseNetworkFlags,
  scaleThrottle,
  throttleFromProfile,
  throttleId,
  timedRun,
} from '../lib/measure.mjs';
import { startServer } from '../lib/serve.mjs';

test('browser timing keeps more than 60 resources, including a tiny preload facade', async (t) => {
  try {
    findChrome();
  } catch {
    t.skip('Chrome/Edge is not installed');
    return;
  }
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'metrics-requests-'));
  for (let i = 0; i < 85; i++) {
    fs.writeFileSync(path.join(dir, `module-${i}.js`), `export default ${i};`);
  }
  fs.writeFileSync(path.join(dir, 'facade.js'), 'import "./module-0.js";');
  fs.writeFileSync(
    path.join(dir, 'entry.js'),
    Array.from({ length: 85 }, (_, i) => `import "./module-${i}.js";`).join('\n') +
      '\ndocument.body.innerHTML = "<h1>Synthetic load test</h1>";',
  );
  fs.writeFileSync(
    path.join(dir, 'index.html'),
    '<!doctype html><link rel="modulepreload" href="/facade.js"><script type="module" src="/entry.js"></script><body></body>',
  );
  const server = await startServer(dir);
  const browser = await launchBrowser({ profileDir: path.join(dir, 'chrome-profile') });
  t.after(async () => {
    await browser.close();
    await server.close();
    fs.rmSync(dir, { recursive: true, force: true });
  });
  const result = await timedRun(browser.cdp, { url: `${server.origin}/`, settleMs: 100 });
  assert.ok(result.resources.length >= 87);
  assert.equal(result.resourceDetailsComplete, true);
  const facade = result.resources.find((r) => r.name === '/facade.js');
  // Chrome versions differ: a modulepreload can report "other" here. Preserve
  // the browser's value and use the CDP source location as separate evidence.
  assert.ok(['link', 'other'].includes(facade.initiatorType));
  assert.equal(facade.type, 'script');
  assert.ok(facade.initiator.type);
  assert.equal(facade.networkEvidence, 'matched');
  assert.equal(typeof facade.requestPriority.initial, 'string');
  assert.equal(typeof facade.requestPriority.last, 'string');
  assert.ok(Array.isArray(facade.requestPriority.changes));
  assert.ok(facade.bytes > 0 && facade.bytes < 100);
  assert.equal('headers' in facade.initiator, false);
  assert.equal(result.lcpElement.text, 'Synthetic load test');
});

test('networkThrottle converts Mbit/s (SI) to CDP bytes per second and keeps the upload ratio', () => {
  const t = networkThrottle(DEFAULT_THROTTLE, { bandwidthMbps: 26, rttMs: 120, cpuRate: 1 });
  assert.equal(t.downloadBps, 3_250_000);
  assert.equal(t.uploadBps, Math.round((3_250_000 * 94_000) / 200_000));
  assert.equal(t.latencyMs, 120);
  assert.equal(t.cpuRate, 1);
  assert.deepEqual(t.network, { bandwidthMbps: 26, rttMs: 120, cpuRate: 1 });
});

test('parseNetworkFlags fills the default RTT and CPU and rejects misuse', () => {
  assert.equal(parseNetworkFlags({}), null);
  assert.deepEqual(parseNetworkFlags({ bandwidth: '26' }), {
    bandwidthMbps: 26,
    rttMs: DEFAULT_THROTTLE.latencyMs,
    cpuRate: DEFAULT_THROTTLE.cpuRate,
  });
  assert.deepEqual(parseNetworkFlags({ bandwidth: '26', rtt: '120', cpu: '1' }), {
    bandwidthMbps: 26,
    rttMs: 120,
    cpuRate: 1,
  });
  assert.throws(() => parseNetworkFlags({ rtt: '120' }), /pass --bandwidth/);
  assert.throws(() => parseNetworkFlags({ cpu: '1' }), /pass --bandwidth/);
  assert.throws(
    () => parseNetworkFlags({ bandwidth: '26', 'net-scale': '8' }),
    /one of --bandwidth or --net-scale/,
  );
  for (const bad of [
    { bandwidth: '0' },
    { bandwidth: 'fast' },
    { bandwidth: '26', rtt: '-1' },
    { bandwidth: '26', cpu: '0.5' },
  ]) {
    assert.throws(() => parseNetworkFlags(bad), /must be a number/, JSON.stringify(bad));
  }
});

test('throttleId gives one identity to a throttle, its pinned profile and a saved state', () => {
  const network = { bandwidthMbps: 26, rttMs: 120, cpuRate: 1 };
  const id = 'bw 26Mbit/s rtt 120ms cpu 1x';
  assert.equal(throttleId(networkThrottle(DEFAULT_THROTTLE, network)), id);
  assert.equal(throttleId({ schemaVersion: 1, network, source: 'flag' }), id);
  assert.equal(throttleId({ runs: 5, network, metrics: {} }), id);
  assert.equal(throttleId(throttleFromProfile({ network })), id);
  // Net-scale identities keep their old spelling (parity manifests store it).
  assert.equal(throttleId(scaleThrottle(DEFAULT_THROTTLE, 8)), 'net x8');
  assert.equal(throttleId({ netScale: 8 }), 'net x8');
  assert.equal(throttleId({ runs: 5, metrics: {} }), 'net x1');
  assert.equal(throttleId(null), 'unthrottled');
  assert.deepEqual(throttleFromProfile({ netScale: 8 }), scaleThrottle(DEFAULT_THROTTLE, 8));
});
