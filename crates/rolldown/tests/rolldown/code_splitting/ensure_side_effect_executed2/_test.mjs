import './dist/a.mjs'
import './dist/b.mjs'

import assert from 'assert';
assert(globalThis.sideEffectExecuted, 'side effect not executed')
