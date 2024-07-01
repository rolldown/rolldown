import assert from 'assert'
import {foo} from './common.js'
assert.deepEqual(foo(), [{default: {}}, undefined])
