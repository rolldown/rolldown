The variables declared in `intro` are scoped to the bundle and won't pollute the global scope. For example, with `format: 'iife'`:

```js
// banner is placed here, outside the IIFE (global scope)
var MyBundle = (function () {
  // intro is placed here, inside the IIFE (local scope)
  var __DEV__ = true; // This won't leak to global scope

  // ... bundle code ...

  // outro is placed here
})();
// footer is placed here
```

#### Examples

##### Polyfilling globalThis

```js
export default {
  output: {
    intro: `
var globalThis = (function() {
  if (typeof globalThis !== 'undefined') return globalThis;
  if (typeof self !== 'undefined') return self;
  if (typeof window !== 'undefined') return window;
  if (typeof global !== 'undefined') return global;
  throw new Error('Unable to locate global object');
})();`,
  },
};
```
