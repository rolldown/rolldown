import { strict as assert } from 'node:assert';
import { read } from './dist/main.js';

// Pre-fix, `import_dep.v` read the parameter: `[undefined, 'param']`, with no error.
assert.deepEqual(read('param'), [1, 'param']);
