#### Example

```js
// input
export { x } from 'external';
```

<!--
  Separate code blocks, not tabs
-->

```js
// CJS output with externalLiveBindings: true
var external = require('external');

Object.defineProperty(exports, 'x', {
  enumerable: true,
  get: function () {
    return external.x;
  },
});
```

<!--
  Separate code blocks, not tabs
-->

```js
// CJS output with externalLiveBindings: false
var external = require('external');

exports.x = external.x;
```
