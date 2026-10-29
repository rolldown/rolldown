import assert from 'node:assert';
import * as path from 'node:path';
import * as cjs from './cjs.js';
import * as reexportCjs from './reexport_cjs.js';

assert.strictEqual('join' in path, true);
assert.strictEqual('a' in cjs, true);
assert.strictEqual('a' in reexportCjs, true);
