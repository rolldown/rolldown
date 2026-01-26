##### What triggers this warning

```js
// main.js
export default function greet() {
  return 'Hello';
}

export const version = '1.0.0';
```

This module uses both a default export and named exports. When consumed in CommonJS environments, users will need to access the default export via `.default`:

```js
// CommonJS consumer
const myLib = require('my-lib');
myLib.default(); // Need to use .default to access the default export
myLib.version; // Named exports work directly
```

To fix this, either use only named exports or set `output.exports: "named"` to acknowledge this behavior:

```js
// Option 1: Use only named exports
export function greet() {
  return 'Hello';
}
export const version = '1.0.0';

// Option 2: Configure output.exports
export default {
  output: {
    exports: 'named', // Suppress the warning
  },
};
```
