import { bar } from './circular-import-constants'
console.log(bar()) // This accesses "foo" before it's initialized