import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import { findChrome, launchBrowser } from '../lib/cdp.mjs';
import { timedRun } from '../lib/measure.mjs';
import { startServer } from '../lib/serve.mjs';

test('browser timing keeps more than 60 resources, including a tiny preload facade', async (t) => {
  try {
    findChrome();
  } catch {
    t.skip('Chrome/Edge is not installed');
    return;
  }
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'metrics-requests-'));
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
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
  t.after(() => server.close());
  const browser = await launchBrowser({ profileDir: path.join(dir, 'chrome-profile') });
  t.after(() => browser.close());
  const result = await timedRun(browser.cdp, { url: `${server.origin}/`, settleMs: 100 });
  assert.ok(result.resources.length >= 87);
  assert.equal(result.resourceDetailsComplete, true);
  const facade = result.resources.find((r) => r.name === '/facade.js');
  // Chrome versions differ: a modulepreload can report "other" here. Preserve
  // the browser's value and use the CDP source location as separate evidence.
  assert.ok(['link', 'other'].includes(facade.initiatorType));
  assert.equal(facade.type, 'script');
  assert.ok(facade.initiator.type);
  assert.ok(facade.bytes > 0 && facade.bytes < 100);
  assert.equal('headers' in facade.initiator, false);
  assert.equal(result.lcpElement.text, 'Synthetic load test');
});
