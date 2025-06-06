import nodeAssert from 'node:assert';
import { foo } from './foo'

nodeAssert.strictEqual(foo, 'foo bar');

export {}