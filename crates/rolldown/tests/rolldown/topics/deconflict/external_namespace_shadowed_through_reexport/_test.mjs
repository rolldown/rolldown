import { strict as assert } from 'node:assert';
import { createRequire } from 'node:module';

// Pre-fix, `node_path.join` read the parameter: `['undefined', 'param']`, with no error.
const { read } = createRequire(import.meta.url)('./dist/main.js');

assert.deepEqual(read('param'), ['function', 'param']);
