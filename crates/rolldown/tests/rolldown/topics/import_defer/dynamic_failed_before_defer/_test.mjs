import assert from 'node:assert';
import { log, result } from './dist/main.js';

assert.strictEqual(result, 'e failed');
assert.deepStrictEqual(log, ['e throws', 'import(d) rejects: e failed', 'import.defer resolves']);
