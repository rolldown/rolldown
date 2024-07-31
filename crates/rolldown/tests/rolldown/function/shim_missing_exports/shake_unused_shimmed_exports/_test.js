import nodeAssert from 'assert'
import { missing } from './dist/main.mjs'

nodeAssert.strictEqual(missing, undefined)