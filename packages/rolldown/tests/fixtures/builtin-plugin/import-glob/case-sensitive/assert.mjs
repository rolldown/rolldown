import assert from 'node:assert';
import { caseInsensitiveModules, caseSensitiveModules } from './dist/main';

assert.strictEqual(caseSensitiveModules['./dir/data-test.js'], 'data-test');
assert.strictEqual(caseSensitiveModules['./dir/DATA-other.js'], undefined);

assert.strictEqual(caseInsensitiveModules['./dir/data-test.js'], 'data-test');
assert.strictEqual(caseInsensitiveModules['./dir/DATA-other.js'], 'DATA-OTHER');
