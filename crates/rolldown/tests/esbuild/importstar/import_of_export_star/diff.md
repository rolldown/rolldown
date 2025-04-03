# Diff
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
import assert from "node:assert";

//#region bar.js
statement();
statement();
statement();
statement();
const bar$1 = 123;

//#endregion
//#region entry.js
assert.equal(bar$1, 123);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,6 +1,6 @@
 statement();
 statement();
 statement();
 statement();
-var bar = 123;
-console.log(bar);
+var bar$1 = 123;
+console.log(bar$1);

```