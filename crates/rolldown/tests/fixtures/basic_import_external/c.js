import * as ext from 'external'
import { a, b } from 'external'
console.log(ext, a, b)

export { a as a1,b as b1 } from 'external'
export * as ext1 from 'external'