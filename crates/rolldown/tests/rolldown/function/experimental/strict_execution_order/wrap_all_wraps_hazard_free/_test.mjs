import assert from 'node:assert';
import fs from 'node:fs';
import { captureConsoleLog } from '../../../../_test_helpers/capture-console-log.mjs';

// Default strict mode is wrap-all: even a hazard-free graph wraps. On-demand leaves the same
// graph unwrapped. Both modes must still reproduce source order.
const logs = await captureConsoleLog(() => import('./dist/main.js'));

assert.deepStrictEqual(logs, ['dep', 'main v']);
const code = fs.readFileSync(new URL('./dist/main.js', import.meta.url), 'utf8');
if (globalThis.__configName === 'on-demand') {
  assert.ok(
    !code.includes('init_') && !code.includes('__esm'),
    'on-demand must not wrap a hazard-free graph',
  );
} else {
  assert.ok(code.includes('init_'), 'wrap-all must wrap');
}
