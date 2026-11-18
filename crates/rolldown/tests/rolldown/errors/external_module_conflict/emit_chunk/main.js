// The `./lib.js` import makes lib.js external. Then the transform hook of trigger.js emits a
// chunk for the same id. The entry module of a chunk is a normal module.
import { value } from './lib.js'
import './trigger.js'

console.log(value)
