# Empty-function call through a side-effect-free barrel

The shape of `expo-modules-core` on web: the barrel also imports the polyfill but is skipped, so `ensure.js` is the only path that still evaluates it.
