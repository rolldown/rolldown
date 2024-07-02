import assert from 'node:assert'
import {foo} from './foo'
assert(foo() === 'foo' && bar() === 'bar');
import {bar} from './bar'
 // This should be hoisted
