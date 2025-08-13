import assert from 'node:assert'
import * as file from './foo.mjs'
import * as dir from './foo/index.mjs'

assert.strictEqual(file.foo, 'foo')
assert.strictEqual(dir.foo, 'foo-index')

import 'trigger-dep'