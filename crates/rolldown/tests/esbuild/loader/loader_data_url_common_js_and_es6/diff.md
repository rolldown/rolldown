# Diff
## /out.js
### esbuild
```js
// x.txt
var require_x = __commonJS({
  "x.txt"(exports, module) {
    module.exports = "data:text/plain;charset=utf-8,x";
  }
});

// y.txt
var y_default = "data:text/plain;charset=utf-8,y";

// entry.js
var x_url = require_x();
console.log(x_url, y_default);
```
### rolldown
```js


//#region y.txt
var y_default = "data:text/plain;charset=utf-8,y";

//#endregion
//#region x.txt
var x_exports, x_default;
var init_x = __esm({ "x.txt"() {
	x_exports = {};
	__export(x_exports, { default: () => x_default });
	x_default = "data:text/plain;charset=utf-8,x";
} });

//#endregion
//#region entry.js
const x_url = (init_x(), __toCommonJS(x_exports));
console.log(x_url, y_default);

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
-        module.exports = "data:text/plain;charset=utf-8,x";
+var y_default = "data:text/plain;charset=utf-8,y";
+var x_exports, x_default;
+var init_x = __esm({
+    "x.txt"() {
+        x_exports = {};
+        __export(x_exports, {
+            default: () => x_default
+        });
+        x_default = "data:text/plain;charset=utf-8,x";
     }
 });
-var y_default = "data:text/plain;charset=utf-8,y";
-var x_url = require_x();
+var x_url = (init_x(), __toCommonJS(x_exports));
 console.log(x_url, y_default);

```