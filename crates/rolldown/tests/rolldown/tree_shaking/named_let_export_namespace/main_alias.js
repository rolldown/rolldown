import assert from 'node:assert';
import { res } from './middle_alias';

assert.strictEqual(res.used, 'used');
