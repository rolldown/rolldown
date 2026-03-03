import assert from 'node:assert';
import * as Babel from './mod';
const { t } = Babel;

assert.strictEqual(t.t, 1);
