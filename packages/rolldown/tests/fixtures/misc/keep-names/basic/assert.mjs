import assert from 'assert';
import { T, t } from './dist/main';

assert.strictEqual(T.name, 'Test');
assert.strictEqual(t.name, 'test');
