import assert from 'node:assert';
import './hmr.js';
import './other.js';

process.on('beforeExit', (code) => {
  if (code !== 0) return;
  // Once from the output of the full build.
  assert.strictEqual(globalThis.__no_reload_after_full_build_runs, 1);
  // `v1` from the output of the full build, then `v2` from the hot update. A reload would run
  // only `v2`, from the new output.
  assert.strictEqual(globalThis.__no_reload_after_full_build_other_runs, 2);
});
