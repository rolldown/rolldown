// The first import makes lib.js external. The second import loads the same id as a normal module.
import { value as externalValue } from './lib.js'
import { value } from './lib'

console.log(value, externalValue)
