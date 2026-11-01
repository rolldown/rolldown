// `./lib` loads lib.js as a normal module first. `./lib.js` is external, with the same id.
import { value } from './lib'
import { value as externalValue } from './lib.js'

console.log(value, externalValue)
