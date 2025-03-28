# Advanced Chunks

Advanced chunks are a powerful feature that allows you manually control the chunking of your code. This is useful when you want to optimize the loading of your application by splitting it into smaller, more manageable pieces.

## Limitations

### Why there's always a `runtime.js` chunk?

tl;dr: If you used `advancedChunks` option, rolldown will forcefully generate a `runtime.js` chunk to ensure that the runtime code is always executed before any other chunks.

The `runtime.js` chunk is a special chunk that **only** contains the runtime code necessary for loading and executing your application. It is generated forcefully by the bundler to ensure that the runtime code is always executed before any other chunks.

Since advanced chunks allows you to move modules between chunks, it's easily to create a circular import in the output code. This can lead to a situation where the runtime code is not executed before the other chunks, causing errors in your application.

A example output code with circular import:

```js
// first.js
import { __esm, __export, init_second, value$1 as value } from './second.js'
var first_exports = {}
__export(first_exports, { value: () => value$1 })
var value$1
var init_first = __esm({
  'first.js'() {
    init_second()
    //...
  },
})
export { first_exports, init_first, value$1 as value }

// main.js
import { __esm, init_second, second_exports } from './second.js'
import { first_exports, init_first } from './first.js'

var init_main = __esm({
  'main.js'() {
    init_first()
    init_second()
    // ...
  },
})

init_main()

// second.js
import { init_first, value } from './first.js'
var __esm = '...'
var __export = '...'

var second_exports = {}
__export(second_exports, { value: () => value$1 })
var value$1
var init_second = __esm({
  'second.js'() {
    init_first()
    //...
  },
})

export { __esm, __export, init_second, second_exports, value$1 }
```

When we run `node ./main.js`, the traversal order of the modules would be `main.js` -> `first.js` -> `second.js`. The module execution order would be `second.js` -> `first.js` -> `main.js`.

`second.js` tries to call `__esm` function before it gets initialized. This will lead to a runtime error which is trying to call `undefined` as a function.

With forcefully generated `runtime.js`, the bundler ensures any chunk that depends on runtime code would first load `runtime.js` before executing itself. This guarantees that the runtime code is always executed before any other chunks, preventing circular import issues.
