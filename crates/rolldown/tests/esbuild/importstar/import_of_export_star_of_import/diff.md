# Diff
## /out.js
### esbuild
```js
// baz.js
var value = 123;

// foo.js
statement();
statement();
statement();
statement();

// entry.js
console.log(value);
```
### rolldown
```js
import assert from "node:assert";

//#region baz.js
const bar = 123;

//#endregion
//#region foo.js
statement();
statement();
statement();
statement();

//#endregion
//#region entry.js
assert.equal(bar, 123);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,6 +1,6 @@
-var value = 123;
+var bar = 123;
 statement();
 statement();
 statement();
 statement();
-console.log(value);
+console.log(bar);

```