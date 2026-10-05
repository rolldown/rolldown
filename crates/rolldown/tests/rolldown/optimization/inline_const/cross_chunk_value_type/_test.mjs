import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const main = readFileSync(new URL('./dist/main.js', import.meta.url), 'utf8');
assert.ok(!main.includes('${'), 'unused primitive interpolation must be removed across chunks');
const { $walks } = await import('./dist/test.js');
assert.equal($walks, 'bE d d%b%lD b/DbE%n&%b/b& m');
