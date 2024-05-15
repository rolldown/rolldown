import * as ext from 'external'
import { a, b } from 'external'
import { a1, b1, ext1 } from './foo'
console.log(ext, a1, b1, ext1, a, b)

export { a, b } from 'external'
