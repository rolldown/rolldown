import assert from 'assert';
import { fn, cls } from './dist/main';

assert.strictEqual(fn.name, 'default');
assert.strictEqual(cls.name, 'default');
