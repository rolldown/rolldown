import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { test } from 'node:test';
import { pathToFileURL } from 'node:url';

import {
  appRolldown,
  checkoutRolldown,
  hasReportMode,
  installCheckoutRolldown,
  reportSupportNote,
} from '../lib/rolldown-support.mjs';

const tmp = () => fs.mkdtempSync(path.join(os.tmpdir(), 'lab-rolldown-'));
const write = (file, text) => {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, text);
};

/** A fake installed package: package.json plus a dist with one declaration file. */
function fakeRolldown(dir, version, { report, binding = false } = {}) {
  write(path.join(dir, 'package.json'), JSON.stringify({ name: 'rolldown', version }));
  write(path.join(dir, 'dist', 'index.mjs'), 'export const VERSION = ' + JSON.stringify(version));
  write(
    path.join(dir, 'dist', 'shared', 'options.d.mts'),
    report
      ? "interface Devtools { mode?: 'full' | 'report' }"
      : "interface Devtools { mode?: 'full' }",
  );
  if (binding) write(path.join(dir, 'dist', 'rolldown-binding.test-arch.node'), 'binary');
}

/** An app whose vite resolves its own nested rolldown, next to a hoisted one of another version. */
function fakeApp({ hoisted, nested }) {
  const app = tmp();
  write(path.join(app, 'package.json'), '{"name":"app"}');
  write(
    path.join(app, 'node_modules', 'vite', 'package.json'),
    '{"name":"vite","version":"8.0.0"}',
  );
  if (hoisted) fakeRolldown(path.join(app, 'node_modules', 'rolldown'), hoisted.version, hoisted);
  if (nested)
    fakeRolldown(
      path.join(app, 'node_modules', 'vite', 'node_modules', 'rolldown'),
      nested.version,
      nested,
    );
  return app;
}

test('hasReportMode reads the declarations under dist', () => {
  const yes = tmp();
  fakeRolldown(yes, '1.2.0', { report: true });
  assert.equal(hasReportMode(yes), true);
  const no = tmp();
  fakeRolldown(no, '1.1.5', { report: false });
  assert.equal(hasReportMode(no), false);
  assert.equal(hasReportMode(tmp()), false);
});

test("appRolldown resolves the copy the app's vite uses, not the hoisted one", () => {
  const app = fakeApp({
    hoisted: { version: '1.2.0', report: true },
    nested: { version: '1.1.5', report: false },
  });
  const found = appRolldown(app);
  assert.equal(found.version, '1.1.5');
  assert.equal(found.reportMode, false);
  assert.equal(found.viteVersion, '8.0.0');
  assert.ok(found.dir.endsWith(path.join('vite', 'node_modules', 'rolldown')));
});

test('appRolldown reports no package when none resolves', () => {
  const app = tmp();
  write(path.join(app, 'package.json'), '{"name":"app"}');
  assert.deepEqual(appRolldown(app), {
    dir: null,
    version: null,
    viteVersion: null,
    reportMode: false,
  });
});

test('reportSupportNote names the cause and the install, and is silent when the mode is there', () => {
  const checkout = tmp();
  fakeRolldown(checkout, '1.2.14', { report: true, binding: true });
  const old = fakeApp({ hoisted: { version: '1.1.5', report: false } });
  const note = reportSupportNote(old, 'node harness.mjs', { checkout });
  assert.match(note, /rolldown 1\.1\.5 at .* has no devtools report mode/);
  assert.match(note, /rolldown-install --app/);
  assert.match(note, /rolldown 1\.2\.14/);
  const fine = fakeApp({ hoisted: { version: '1.2.14', report: true } });
  assert.equal(reportSupportNote(fine, 'node harness.mjs', { checkout }), null);
});

test('reportSupportNote asks for a build when the checkout has none', () => {
  const checkout = tmp();
  write(path.join(checkout, 'package.json'), '{"name":"rolldown","version":"1.2.14"}');
  const app = fakeApp({ hoisted: { version: '1.1.5', report: false } });
  const note = reportSupportNote(app, 'node harness.mjs', { checkout });
  assert.match(note, /is not built: run `just build-rolldown-release`/);
  assert.equal(checkoutRolldown(checkout).built, false);
});

test('reportSupportNote points at --from when no package sits next to the lab', () => {
  const app = fakeApp({ hoisted: { version: '1.1.5', report: false } });
  const note = reportSupportNote(app, 'node harness.mjs', { checkout: path.join(tmp(), 'absent') });
  assert.match(note, /No rolldown package sits next to the lab/);
  assert.match(note, /--from <that packages\/rolldown>/);
});

test('installCheckoutRolldown replaces the resolved copy and keeps the binding', () => {
  const checkout = tmp();
  fakeRolldown(checkout, '1.2.14', { report: true, binding: true });
  write(path.join(checkout, 'bin', 'cli.mjs'), '// cli');
  const app = fakeApp({ hoisted: { version: '1.1.5', report: false } });
  const stale = path.join(app, 'node_modules', 'rolldown', 'dist', 'stale.mjs');
  write(stale, '// from the old version');
  const result = installCheckoutRolldown(app, { checkout });
  assert.equal(result.before, '1.1.5');
  assert.equal(result.after.version, '1.2.14');
  assert.equal(result.after.reportMode, true);
  assert.equal(fs.existsSync(stale), false, 'the old dist is removed, not merged');
  assert.ok(fs.existsSync(path.join(result.dir, 'dist', 'rolldown-binding.test-arch.node')));
  assert.ok(fs.existsSync(path.join(result.dir, 'bin', 'cli.mjs')));
});

test('installCheckoutRolldown refuses an unbuilt checkout', () => {
  const checkout = tmp();
  write(path.join(checkout, 'package.json'), '{"name":"rolldown","version":"1.2.14"}');
  const app = fakeApp({ hoisted: { version: '1.1.5', report: false } });
  assert.throws(() => installCheckoutRolldown(app, { checkout }), /is not built/);
});

test('an installed lab (under node_modules) has no checkout next to it', async () => {
  const lib = path.join(tmp(), 'node_modules', 'rolldown-metric-lab', 'lib');
  fs.mkdirSync(lib, { recursive: true });
  fs.copyFileSync(
    new URL('../lib/rolldown-support.mjs', import.meta.url),
    path.join(lib, 'rolldown-support.mjs'),
  );
  const installed = await import(pathToFileURL(path.join(lib, 'rolldown-support.mjs')).href);
  assert.equal(installed.CHECKOUT_ROLLDOWN, null);
  assert.equal(installed.checkoutRolldown(), null);
  const app = fakeApp({ hoisted: { version: '1.1.5', report: false } });
  assert.match(
    installed.reportSupportNote(app, 'npx @rolldown/perf-kit'),
    /--from <that packages\/rolldown>/,
  );
  assert.throws(() => installed.installCheckoutRolldown(app), /pass --from/);
});
