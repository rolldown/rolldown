This means `outro` code has access to the bundle's internal scope and can reference private variables and functions. For example, with `format: 'iife'`:

```js
// banner is placed here
var MyBundle = (function () {
  // intro is placed here
  var privateVar = 'internal'; // Only accessible inside IIFE

  // ... bundle code ...

  // outro is placed here - can access privateVar
  console.log(privateVar); // Works!
})();
// footer is placed here (global scope) - cannot access privateVar
```

#### Examples

##### Freeze exports

```js
export default {
  output: {
    format: 'iife',
    name: 'MyLib',
    outro: `
// Freeze the exported API to prevent modifications
if (typeof Object.freeze === 'function') {
  Object.freeze(exports);
}`,
  },
};
```
