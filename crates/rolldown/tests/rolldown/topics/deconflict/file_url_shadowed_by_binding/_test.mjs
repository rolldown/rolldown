import { strict as assert } from 'node:assert';
import { href, other } from './dist/main.js';
assert.match(href(1), /asset.*\.txt$/);
assert.equal(other, 'local');
