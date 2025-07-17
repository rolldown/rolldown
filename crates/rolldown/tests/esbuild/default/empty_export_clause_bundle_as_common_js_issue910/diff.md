# Diff
## /out.js
### esbuild
```js
// types.mjs
var types_exports = {};
var init_types = __esm({
  "types.mjs"() {
  }
});

// entry.js
console.log((init_types(), __toCommonJS(types_exports)));
```
### rolldown
```js
import assert from "node:assert";

// HIDDEN [rolldown:runtime]
//#region types.mjs
var types_exports = {};
var init_types = __esm({ "types.mjs": (() => {}) });

//#endregion
//#region entry.js
assert.deepEqual((init_types(), __toCommonJS(types_exports)), {});

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,5 +1,5 @@
 var types_exports = {};
 var init_types = __esm({
-    "types.mjs"() {}
+    "types.mjs": () => {}
 });
 console.log((init_types(), __toCommonJS(types_exports)));

```