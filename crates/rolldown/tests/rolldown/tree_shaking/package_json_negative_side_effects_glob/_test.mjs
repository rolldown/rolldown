import assert from 'node:assert/strict';
import fs from 'node:fs';

const output = fs.readFileSync(new URL('./dist/main.js', import.meta.url), 'utf8');

assert.match(output, /kept-side-effect-marker/);
assert.match(output, /excluded-side-effect-marker/);
assert.doesNotMatch(output, /unused-side-effect-marker/);
