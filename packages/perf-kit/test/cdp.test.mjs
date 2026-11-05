// The browser handle: close() must outlive Chrome. Every caller removes the profile
// directory right after close(), and Chrome writes into it until it has exited.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import { findChrome, launchBrowser } from '../lib/cdp.mjs';

test('close() returns only after the Chrome process exited', async (t) => {
  try {
    findChrome();
  } catch {
    t.skip('Chrome/Edge is not installed');
    return;
  }
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'metrics-cdp-'));
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  const profileDir = path.join(dir, 'chrome-profile');
  const browser = await launchBrowser({ profileDir });
  assert.ok(Number.isInteger(browser.pid) && browser.pid > 0);
  await browser.close();
  // Signal 0 probes the process: an exited (and reaped) process answers ESRCH.
  assert.throws(() => process.kill(browser.pid, 0), { code: 'ESRCH' });
  // The reason the wait exists: the directory can be removed in one call now.
  fs.rmSync(profileDir, { recursive: true, force: true });
  assert.equal(fs.existsSync(profileDir), false);
});
