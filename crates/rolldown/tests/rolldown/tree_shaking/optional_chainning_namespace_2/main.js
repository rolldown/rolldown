import assert from 'node:assert';
import * as foo from './foo';

assert.strictEqual(foo?.bar?.something?.foo, undefined);
