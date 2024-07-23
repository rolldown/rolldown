import assert from 'node:assert'
import {foo} from './common.js'
assert.deepEqual(foo(), [{default: {}}, undefined])
