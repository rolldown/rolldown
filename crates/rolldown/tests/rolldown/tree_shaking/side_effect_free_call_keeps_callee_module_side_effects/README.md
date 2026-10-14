# Empty-function call keeps its module's side-effectful imports

`ensure.js` is side-effect-free (`sideEffects` lists only `./polyfill/*.js`), imports the polyfill, and exports an empty function. `reader.js` calls `ensure()` and then reads what the polyfill set.

Rolldown may drop the call to the empty function, but it must still evaluate `ensure.js`, and so the polyfill, before `reader.js`. webpack and esbuild keep the polyfill here.
