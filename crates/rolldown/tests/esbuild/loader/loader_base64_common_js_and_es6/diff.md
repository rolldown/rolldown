# Diff
## /out.js
### esbuild
```js
// x.b64
var require_x = __commonJS({
  "x.b64"(exports, module) {
    module.exports = "eA==";
  }
});

// y.b64
var y_default = "eQ==";

// entry.js
var x_b64 = require_x();
console.log(x_b64, y_default);
```
### rolldown
```js
import { default as assert } from "node:assert";


//#region y.b64
var y_default = "eQ==";

//#endregion
//#region x.b64
var x_exports, x_default;
var init_x = __esm({ "x.b64"() {
	x_exports = {};
	__export(x_exports, { default: () => x_default });
	x_default = "eA==";
} });

//#endregion
//#region entry.js
const x_b64 = (init_x(), __toCommonJS(x_exports));
assert.deepEqual(x_b64, { default: "eA==" });
assert.equal(y_default, "eQ==");

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,8 +1,13 @@
-var require_x = __commonJS({
-    "x.b64"(exports, module) {
-        module.exports = "eA==";
+var y_default = "eQ==";
+var x_exports, x_default;
+var init_x = __esm({
+    "x.b64"() {
+        x_exports = {};
+        __export(x_exports, {
+            default: () => x_default
+        });
+        x_default = "eA==";
     }
 });
-var y_default = "eQ==";
-var x_b64 = require_x();
+var x_b64 = (init_x(), __toCommonJS(x_exports));
 console.log(x_b64, y_default);

```