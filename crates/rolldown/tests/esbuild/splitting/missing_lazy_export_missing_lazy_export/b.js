import assert from 'node:assert'
import {bar} from './common.js'
assert.deepEqual(bar(), [undefined])
