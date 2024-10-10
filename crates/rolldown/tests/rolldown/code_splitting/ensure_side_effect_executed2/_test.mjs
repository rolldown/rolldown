import './dist/a.js'
import './dist/b.js'

import assert from 'assert';
assert(globalThis.sideEffectExecuted, 'side effect not executed')
