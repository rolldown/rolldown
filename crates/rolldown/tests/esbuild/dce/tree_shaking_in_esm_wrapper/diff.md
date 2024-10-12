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
import { default as assert } from "node:assert";


//#region lib.js
var keep1, keep2;
var init_lib = __esm({ "lib.js"() {
	keep1 = () => "keep1";
	keep2 = () => "keep2";
} });

//#endregion
//#region cjs.js
var cjs_exports, cjs_default;
var init_cjs = __esm({ "cjs.js"() {
	cjs_exports = {};
	__export(cjs_exports, { default: () => cjs_default });
	init_lib();
	cjs_default = keep2();
} });

//#endregion
//#region entry.js
init_lib();
assert.equal(keep1(), "keep1");
assert.deepEqual((init_cjs(), __toCommonJS(cjs_exports)).default, "keep2");

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -4,17 +4,17 @@
         keep1 = () => "keep1";
         keep2 = () => "keep2";
     }
 });
-var cjs_exports = {};
-__export(cjs_exports, {
-    default: () => cjs_default
-});
-var cjs_default;
+var cjs_exports, cjs_default;
 var init_cjs = __esm({
     "cjs.js"() {
+        cjs_exports = {};
+        __export(cjs_exports, {
+            default: () => cjs_default
+        });
         init_lib();
         cjs_default = keep2();
     }
 });
 init_lib();
-console.log(keep1(), (init_cjs(), __toCommonJS(cjs_exports)));
+console.log(keep1(), (init_cjs(), __toCommonJS(cjs_exports)).default);

```