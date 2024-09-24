## /out.js
### esbuild
```js
// foo.js
var foo_exports = {};
__export(foo_exports, {
  bar_ns: () => bar_exports
});

// bar.js
var bar_exports = {};
__export(bar_exports, {
  bar: () => bar
});
var bar = 123;

// entry.js
console.log(foo_exports);
```
### rolldown
```js
import { default as assert } from "node:assert";


//#region bar.js
var bar_exports = {};
__export(bar_exports, { bar: () => bar });
const bar = 123;

//#endregion
//#region foo.js
var foo_exports = {};
__export(foo_exports, { bar_ns: () => bar_exports });

//#endregion
//#region entry.js
console.log(foo_exports);
assert.deepEqual(foo_exports, { bar_ns: { bar: 123 } });

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry_js.mjs
@@ -1,6 +1,7 @@
-var foo_exports = {};
-__export(foo_exports, { bar_ns: () => bar_exports });
 var bar_exports = {};
 __export(bar_exports, { bar: () => bar });
 var bar = 123;
+var foo_exports = {};
+__export(foo_exports, { bar_ns: () => bar_exports });
+console.log(foo_exports);
 console.log(foo_exports);
\ No newline at end of file

```
