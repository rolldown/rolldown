# Diff
## /out.js
### esbuild
```js
// lib.js
var keep1, keep2;
var init_lib = __esm({
  "lib.js"() {
    keep1 = () => "keep1";
    keep2 = () => "keep2";
  }
});

// cjs.js
var cjs_exports = {};
__export(cjs_exports, {
  default: () => cjs_default
});
var cjs_default;
var init_cjs = __esm({
  "cjs.js"() {
    init_lib();
    cjs_default = keep2();
  }
});

// entry.js
init_lib();
console.log(keep1(), (init_cjs(), __toCommonJS(cjs_exports)));
```
### rolldown
```js
import assert from "node:assert";

// HIDDEN [rolldown:runtime]
//#region lib.js
var keep1, keep2;
var init_lib = __esm({ "lib.js": (() => {
	keep1 = () => "keep1";
	keep2 = () => "keep2";
}) });

//#endregion
//#region cjs.js
var cjs_exports = {};
__export(cjs_exports, { default: () => cjs_default });
var cjs_default;
var init_cjs = __esm({ "cjs.js": (() => {
	init_lib();
	cjs_default = keep2();
}) });

//#endregion
//#region entry.js
init_lib();
assert.equal(keep1(), "keep1");
assert.deepEqual((init_cjs(), __toCommonJS(cjs_exports)), { default: "keep2" });

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,7 +1,7 @@
 var keep1, keep2;
 var init_lib = __esm({
-    "lib.js"() {
+    "lib.js": () => {
         keep1 = () => "keep1";
         keep2 = () => "keep2";
     }
 });
@@ -10,9 +10,9 @@
     default: () => cjs_default
 });
 var cjs_default;
 var init_cjs = __esm({
-    "cjs.js"() {
+    "cjs.js": () => {
         init_lib();
         cjs_default = keep2();
     }
 });

```