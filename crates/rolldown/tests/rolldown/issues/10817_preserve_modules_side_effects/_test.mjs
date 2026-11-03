import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

const dist = join(import.meta.dirname, 'dist');
const main = readFileSync(join(dist, 'main.js'), 'utf8');

assert.doesNotMatch(main, /(?:import|require).*derived\.js/);
assert.equal(
  existsSync(join(dist, 'derived.js')),
  false,
  'dead derived module must not be emitted',
);
