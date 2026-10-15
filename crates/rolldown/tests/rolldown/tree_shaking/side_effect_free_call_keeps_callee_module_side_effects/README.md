# Empty-function call keeps its module's side-effectful imports

`ensure.js` is side-effect-free, imports the polyfill and exports an empty `ensure()`. Dropping the call to `ensure()` must not drop `ensure.js`, or `reader.js` runs before the polyfill.
