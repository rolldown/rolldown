import assert from 'node:assert';
import { res } from './middle';

assert.strictEqual(res.used, 'used');
