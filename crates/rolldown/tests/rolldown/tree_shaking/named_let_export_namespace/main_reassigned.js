import assert from 'node:assert';
import { res } from './middle_reassigned';

assert.strictEqual(res.used, 'overridden');
