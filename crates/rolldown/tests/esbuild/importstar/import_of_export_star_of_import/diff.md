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
import { default as assert } from "node:assert";

//#region baz.js
const value = 123;

//#endregion
//#region foo.js
statement();
statement();
statement();
statement();

//#endregion
//#region entry.js
assert.equal(value, 123);

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry_js.mjs
@@ -1,5 +1,5 @@
-var value = 123;
+const value = 123;
 statement();
 statement();
 statement();
 statement();

```
