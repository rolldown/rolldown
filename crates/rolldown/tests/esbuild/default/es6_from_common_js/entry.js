import assert from 'node:assert'
import {foo} from './foo'
assert.equal(foo(), 'foo' );
assert.equal(bar() ,'bar')
import {bar} from './bar'
 // This should be hoisted
