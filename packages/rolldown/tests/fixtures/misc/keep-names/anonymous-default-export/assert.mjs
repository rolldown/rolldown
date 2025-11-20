import assert from 'assert';
import { cls, fn } from './dist/main';

assert.strictEqual(fn.name, 'default');
assert.strictEqual(cls.name, 'default');
