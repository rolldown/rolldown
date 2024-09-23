## /out.js
### esbuild
```js
// bar.js
statement();
statement();
statement();
statement();
var bar = 123;

// entry.js
console.log(bar);
```
### rolldown
```js
import { default as assert } from "node:assert";

//#region bar.js
statement();
statement();
statement();
statement();
const bar = 123;

//#endregion
//#region entry.js
assert.equal(bar, 123);

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry_js.mjs
@@ -1,6 +1,6 @@
 statement();
 statement();
 statement();
 statement();
-var bar = 123;
+const bar = 123;
 console.log(bar);
\ No newline at end of file

```
