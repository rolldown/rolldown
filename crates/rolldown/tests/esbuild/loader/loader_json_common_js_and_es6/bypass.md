# Reason
1. esbuild will inline declaration 
2. sub optimal
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
var y1 = true;
var y2 = false;
var y_default = {
	y1,
	y2
};

//#endregion
//#region z.json
var small = "some small text";
var fi = "test keyword imports";

//#endregion
//#region x.json
var require_x = __commonJS({ "x.json"(exports, module) {
	module.exports = { "x": true };
} });

//#endregion
//#region entry.js
const x_json = require_x();
console.log(x_json, y_default, small, fi);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,15 +1,17 @@
+var y1 = true;
+var y2 = false;
+var y_default = {
+    y1,
+    y2
+};
+var small = "some small text";
+var fi = "test keyword imports";
 var require_x = __commonJS({
     "x.json"(exports, module) {
         module.exports = {
-            x: true
+            "x": true
         };
     }
 });
-var y_default = {
-    y1: true,
-    y2: false
-};
-var small = "some small text";
-var if2 = "test keyword imports";
 var x_json = require_x();
-console.log(x_json, y_default, small, if2);
+console.log(x_json, y_default, small, fi);

```