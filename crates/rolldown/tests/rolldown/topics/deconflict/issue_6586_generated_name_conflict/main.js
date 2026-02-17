import assert from 'assert';
import { a$1 as aFromModule } from './a.js';

// Top-level variable `a` - this causes `a` from a.js to be renamed to `a$1`
const a = 'from-main';

// This function has a parameter named `a$1` in the ORIGINAL source.
// After bundling, `a` from a.js becomes `a$1` at top-level.
// The parameter `a$1` must be renamed to `a$1$1` to avoid incorrectly
// shadowing the renamed top-level `a$1`.
function testGeneratedNameConflict(a$1) {
  // `a$1` here should refer to the parameter, not the top-level renamed variable
  // `aFromModule` should still correctly reference the imported value
  return {
    param: a$1,
    topLevelA: a,
    importedA: aFromModule,
  };
}

const result = testGeneratedNameConflict('param-value');
console.log(`aFromModule: `, aFromModule);

assert.strictEqual(result.param, 'param-value');
assert.strictEqual(result.topLevelA, 'from-main');
assert.strictEqual(result.importedA, 'from-a-module');
export { a, testGeneratedNameConflict };
