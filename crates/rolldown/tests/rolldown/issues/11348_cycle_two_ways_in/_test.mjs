import assert from 'node:assert';
import { execFileSync } from 'node:child_process';
import path from 'node:path';

// Each entry runs in its own process, so each entry enters the cycle first.
const run = (entry) =>
  execFileSync('node', [path.join(import.meta.dirname, 'dist', entry)], {
    encoding: 'utf-8',
  }).trim();

// Node.js runs a.cjs before after.js. Here the cycle runs at the `require_a()` call in the body of
// main-a.js, after the other modules that main-a.js imports. This is a known limit. See the caveat
// in docs/in-depth/bundling-cjs.md.
assert.strictEqual(run('main-a.js'), 'after,a,b a saw b.name=b');
assert.strictEqual(run('main-b.js'), 'b,a b saw a.name=a');
