import assert from 'node:assert';
import getThisDefault, { getThis, 'get-this' as getThisByString } from './lib';

// A plain call of an imported function must not get the module's exports object as `this`.
assert.strictEqual(getThis(), undefined);
assert.strictEqual(getThis?.(), undefined);
assert.strictEqual(getThis`x`, undefined);
assert.strictEqual(getThisDefault(), undefined);
assert.strictEqual(getThisByString(), undefined);

import 'trigger-dep';
