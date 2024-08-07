import './dist/entry_js.mjs'
import './dist/entry2_js.mjs'

import assert from 'assert';
assert(globalThis.sideEffectExecuted, 'side effect not executed')
