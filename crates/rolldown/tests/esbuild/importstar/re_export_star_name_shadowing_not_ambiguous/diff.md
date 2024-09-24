## /out.js
### esbuild
```js
// a.js
var x = 1;

// entry.js
console.log(x);
```
### rolldown
```js
import { default as assert } from "node:assert";

//#region a.js
let x = 1;

//#endregion
//#region entry.js
assert.equal(x, 1);

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry_js.mjs

```
