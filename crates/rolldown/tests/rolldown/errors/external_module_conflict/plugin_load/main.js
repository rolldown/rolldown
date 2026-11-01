// The `./lib.js` import makes lib.js external. Then the transform hook of trigger.js calls
// `this.load` for the same id, and `this.load` loads a normal module.
import { value } from './lib.js'
import './trigger.js'

console.log(value)
