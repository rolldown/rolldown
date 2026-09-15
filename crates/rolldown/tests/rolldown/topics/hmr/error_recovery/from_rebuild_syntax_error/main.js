// Step 0 breaks dep.js with a syntax error; the failed scan merges nothing.
// Step 1 restores the original bytes and must still ship a patch: after an
// errored build the noop suppression is off (`last_build_errored`), so clients
// stuck on the error overlay see the recovery.
import { value } from './dep'
console.log(value)
