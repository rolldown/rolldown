# Empty-function call through a side-effect-free barrel

The shape of `expo-modules-core` on web. The package marks only `./polyfill/*.js` as side-effectful. Its barrel `index.js` imports the polyfill, but the barrel is side-effect-free and only re-exports, so it is skipped. `reader.js` still evaluates the polyfill through `ensure.js`, because it calls `ensure()`.

`ensure()` is an empty function, so rolldown may drop the call. It must not drop the evaluation of `ensure.js` with it, or `reader.js` runs before the polyfill.
