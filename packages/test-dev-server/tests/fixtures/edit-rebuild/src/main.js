import assert from 'node:assert';
import { value as fooValue } from './foo';

assert.strictEqual(fooValue, 'foo');
