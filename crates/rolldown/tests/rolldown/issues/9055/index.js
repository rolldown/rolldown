import { greet } from './cjs-dependency.cjs';
import { strict as assert } from 'node:assert';

// Regression assertion: prior to the fix, the bundled `cjs-dependency.cjs` closure emitted
// `const require_greet = require_greet();` where the local LHS shadowed the synthesized
// chunk-scope `require_greet` wrapper, producing a TDZ ReferenceError at import time.
assert.strictEqual(typeof greet, 'function');
assert.strictEqual(greet(), 'hello from cjs-dependency');
