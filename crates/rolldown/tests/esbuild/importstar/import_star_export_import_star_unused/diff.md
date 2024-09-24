## /out.js
### esbuild
```js
// entry.js
var foo = 234;
console.log(foo);
```
### rolldown
```js
import { default as assert } from "node:assert";

//#region entry.js
let foo = 234;
assert.equal(foo, 234);

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry_js.mjs

```
