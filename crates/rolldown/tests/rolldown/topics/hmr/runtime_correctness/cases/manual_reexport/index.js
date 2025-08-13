import assert from "node:assert"

import { Globals, value } from './barrel'

assert.strictEqual(value, 'lib');
assert.strictEqual(Globals, Object);

import 'trigger-dep'