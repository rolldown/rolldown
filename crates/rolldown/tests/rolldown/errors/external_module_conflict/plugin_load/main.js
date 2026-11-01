// `./lib.js` is external. The plugin then loads the same id as a normal module from the
// transform hook of trigger.js.
import { value } from './lib.js'
import './trigger.js'

console.log(value)
