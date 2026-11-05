// The first import loads lib.js as a normal module. The second import makes the same id external.
import { value } from './lib'
import { value as externalValue } from './lib.js'

console.log(value, externalValue)
