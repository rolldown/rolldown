# Runtime Helpers

## `__commonJS`: factory reference must be released after initialization

The `__commonJS` helper retains a closure over `cb` (the factory function). After the first call, `mod` is set and `cb` is never accessed again — but without an explicit release, the closure keeps `cb` alive indefinitely.

This is a memory leak in long-lived Node.js processes (e.g. SSR servers that load bundles via `vm.createContext`). Each factory function can contain thousands of lines of compiled library code. In a typical bundle with hundreds of CJS modules, all factory functions remain in the heap permanently even though they are never called again after initialization.

The fix is to set `cb = null` after the factory has been called:

```js
// Before: cb is retained in the closure forever
var __commonJS = (cb, mod) =>
  function __require() {
    return (
      mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod),
      mod.exports
    );
  };

// After: cb is released after first call, eligible for GC
var __commonJS = (cb, mod) =>
  function __require() {
    return (
      mod || ((0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod), (cb = null)),
      mod.exports
    );
  };
```

Reference: https://github.com/rolldown/rolldown/issues/9063
