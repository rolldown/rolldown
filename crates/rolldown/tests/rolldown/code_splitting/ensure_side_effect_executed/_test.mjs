import './dist/entry_js.js'
import './dist/entry2_js.js'

import assert from 'assert';
assert(globalThis.sideEffectExecuted, 'side effect not executed')
