import assert from 'node:assert';
import cls from './dep-class';
import fn from './dep-function';

assert.strictEqual(fn.name, 'default');
assert.strictEqual(cls.name, 'default');

export { cls, fn };
