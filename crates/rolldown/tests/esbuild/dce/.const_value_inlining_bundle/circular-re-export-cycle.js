export const baz = 0
import { bar } from './circular-re-export-constants'
console.log(bar()) // This accesses "foo" before it's initialized