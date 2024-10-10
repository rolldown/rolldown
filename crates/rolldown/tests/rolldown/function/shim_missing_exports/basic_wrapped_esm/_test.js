import nodeAssert from 'assert'
import { missing } from './dist/main.js'

nodeAssert.strictEqual(missing, undefined)
