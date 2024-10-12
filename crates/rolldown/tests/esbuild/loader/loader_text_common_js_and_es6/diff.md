# Diff
## /out.js
### esbuild
```js
// x.txt
var require_x = __commonJS({
  "x.txt"(exports, module) {
    module.exports = "x";
  }
});

// y.txt
var y_default = "y";

// entry.js
var x_txt = require_x();
console.log(x_txt, y_default);
```
### rolldown
```js
import { default as assert } from "node:assert";


//#region y.txt
var y_default = "y";

//#endregion
//#region x.txt
var x_exports, x_default;
var init_x = __esm({ "x.txt"() {
	x_exports = {};
	__export(x_exports, { default: () => x_default });
	x_default = "x";
} });

//#endregion
//#region entry.js
const x_txt = (init_x(), __toCommonJS(x_exports));
assert.deepEqual(x_txt, { default: "x" });
assert.equal(y_default, "y");

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,8 +1,13 @@
-var require_x = __commonJS({
-    "x.txt"(exports, module) {
-        module.exports = "x";
+var y_default = "y";
+var x_exports, x_default;
+var init_x = __esm({
+    "x.txt"() {
+        x_exports = {};
+        __export(x_exports, {
+            default: () => x_default
+        });
+        x_default = "x";
     }
 });
-var y_default = "y";
-var x_txt = require_x();
+var x_txt = (init_x(), __toCommonJS(x_exports));
 console.log(x_txt, y_default);

```