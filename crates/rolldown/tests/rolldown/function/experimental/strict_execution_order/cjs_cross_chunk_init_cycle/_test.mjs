import assert from 'node:assert';
import { createRequire } from 'node:module';
import { readdirSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

// Strict-execution-order + `format: cjs` with code splitting: `g.js` (m1, m2) and `m3.js` form a
// cross-chunk cycle, so `g.js` exports its `init_m2` wrapper to `m3.js` and vice versa. The order
// wrappers self-rebind on first call, so their cross-chunk exports MUST be live getters. A future
// degradation to a value snapshot (`exports.init_m2 = init_m2`) would freeze the pre-rebind
// function and re-run a module body on every later call — that would surface here as a duplicate
// entry in `__log` (or as unbounded recursion through the cycle). Today these exports render
// through the common-chunk arm of `render_chunk_exports` (entry chunks holding wrapped modules
// are split into facades, so `e0.js`/`e1.js` export no wrappers themselves); this fixture pins
// the emitted shape wherever it renders. It is also the first executed strict+cjs coverage (the
// invariants harness forces ESM), so it pins that strict+cjs runs at all.

const require = createRequire(import.meta.url);
const distDir = join(dirname(fileURLToPath(import.meta.url)), 'dist');
// Full `minify` mangles the local `init_*` names, so the name-based structural greps below only
// apply to the unminified configs; under minify, single execution + correct values (section (b))
// are the load-bearing assertions that the self-rebinding wrapper still runs each body once.
const isMinified = ['minify', 'minify-wrap-all'].includes(globalThis.__configName);

const combined = readdirSync(distDir)
  .filter((name) => name.endsWith('.js'))
  .map((name) => readFileSync(join(distDir, name), 'utf8'))
  .join('\n');

if (isMinified) {
  // (a-min) Loose, name-agnostic pin that the hoisted self-rebinding form survived compression: a
  // function that reassigns its own binding and immediately calls it
  // (`function X(){return(X=...)()}`).
  assert.match(
    combined,
    /function (\w+)\(\)\{return\(?\1=/,
    'the self-rebinding order wrapper form must survive minification',
  );
} else {
  // (a) Structural pin: every cross-chunk `init_*` export must be a live getter, never a value
  // snapshot. Read the emitted chunks directly so the guard fails loudly even if someone
  // regenerates the artifacts snapshot. Written to survive `minifyInternalExports` (which shortens
  // the export KEY, e.g. `init_m2` -> `i`, but keeps the local `init_m2` name), so it matches on
  // the local wrapper name rather than the export key.
  //
  // The order wrapper is returned from a getter body (`get: function() { return init_m2; }`).
  assert.match(
    combined,
    /return init_m\d+;/,
    'expected at least one order wrapper to be exported as a live getter',
  );
  // The degradation this guards against: a value snapshot of the wrapper (`exports.<key> =
  // init_m2`), which would freeze the pre-rebind function regardless of how the export key is named.
  assert.doesNotMatch(
    combined,
    /\bexports\.\w+\s*=\s*init_\w+/,
    'an order wrapper must never be exported as a value snapshot (`exports.x = init_x`)',
  );
  // The self-rebinding wrapper form must survive so first-call rebinding caches the module body.
  assert.match(
    combined,
    /function init_m2\(\)\s*\{\s*return \(init_m2 =/,
    'the hoisted self-rebinding order wrapper form must be preserved',
  );
}

// (b) Behavioural pin: loading both entries must run every module body EXACTLY once and produce
// the correct values.
globalThis.__log = [];
const e0 = require('./dist/e0.js');
const e1 = require('./dist/e1.js');

for (const name of ['m0', 'm1', 'm2', 'm3']) {
  const count = globalThis.__log.filter((entry) => entry === name).length;
  assert.strictEqual(count, 1, `module ${name} body must execute exactly once, ran ${count} times`);
}

assert.strictEqual(e0.v0, 10, 'e0 should re-export m0.v0');
assert.strictEqual(e1.v1, 1, 'e1 should re-export m1.v1');
// v2 = v0 + 2 proves m2 observed m0's initialized export under strict order.
assert.strictEqual(e1.v2, 12, 'e1.v2 must equal m0.v0 + 2, proving in-order initialization');

delete globalThis.__log;
