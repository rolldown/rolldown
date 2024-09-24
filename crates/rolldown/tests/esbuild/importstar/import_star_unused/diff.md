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
assert(foo === 234);

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry_js.mjs
@@ -1,2 +1,2 @@
 var foo = 234;
-console.log(foo);
\ No newline at end of file
+assert(foo === 234);
\ No newline at end of file

```
