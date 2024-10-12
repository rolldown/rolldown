# Diff
## /out.js
### esbuild
```js
// x.json
var require_x = __commonJS({
  "x.json"(exports, module) {
    module.exports = { x: true };
  }
});

// y.json
var y_default = { y1: true, y2: false };

// z.json
var small = "some small text";
var if2 = "test keyword imports";

// entry.js
var x_json = require_x();
console.log(x_json, y_default, small, if2);
```
### rolldown
```js


//#region y.json
const y1 = true;
const y2 = false;
var y_default = {
	y1,
	y2
};

//#endregion
//#region z.json
const small = "some small text";
const key_2 = "test keyword imports";

//#endregion
//#region x.json
var x_exports, x, x_default;
var init_x = __esm({ "x.json"() {
	x_exports = {};
	__export(x_exports, {
		default: () => x_default,
		x: () => x
	});
	x = true;
	x_default = { x };
} });

//#endregion
//#region entry.js
const x_json = (init_x(), __toCommonJS(x_exports).default);
console.log(x_json, y_default, small, key_2);

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,15 +1,24 @@
-var require_x = __commonJS({
-    "x.json"(exports, module) {
-        module.exports = {
-            x: true
-        };
-    }
-});
+var y1 = true;
+var y2 = false;
 var y_default = {
-    y1: true,
-    y2: false
+    y1,
+    y2
 };
 var small = "some small text";
-var if2 = "test keyword imports";
-var x_json = require_x();
-console.log(x_json, y_default, small, if2);
+var key_2 = "test keyword imports";
+var x_exports, x, x_default;
+var init_x = __esm({
+    "x.json"() {
+        x_exports = {};
+        __export(x_exports, {
+            default: () => x_default,
+            x: () => x
+        });
+        x = true;
+        x_default = {
+            x
+        };
+    }
+});
+var x_json = (init_x(), __toCommonJS(x_exports).default);
+console.log(x_json, y_default, small, key_2);

```