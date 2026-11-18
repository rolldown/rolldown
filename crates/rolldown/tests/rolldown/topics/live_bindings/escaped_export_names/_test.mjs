import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const main = require('./dist/main.js');
const name = '\n"\\\'';

assert.equal(typeof Object.getOwnPropertyDescriptor(main, name)?.get, 'function');
assert.equal(main[name], 1);
main.update();
assert.equal(main[name], 2);
