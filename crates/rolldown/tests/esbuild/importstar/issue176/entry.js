import assert from 'node:assert'
import * as things from './folders'
assert(Object.keys(JSON.stringify(things)), 2)
