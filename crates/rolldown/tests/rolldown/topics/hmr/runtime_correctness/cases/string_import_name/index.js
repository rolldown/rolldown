import assert from 'node:assert';
import { 'a-b' as direct } from './lib';
import { value as reexported } from './reexport';

assert.strictEqual(direct, 'lib');
assert.strictEqual(reexported, 'lib');

import 'trigger-dep';
