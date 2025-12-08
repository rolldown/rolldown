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
// HIDDEN [rolldown:runtime]
//#region y.json
var y_default = {
	y1: true,
	y2: false
};

//#endregion
//#region z.json
var small = "some small text";
var _if = "test keyword imports";

//#endregion
//#region x.json
var require_x = /* @__PURE__ */ __commonJSMin(((exports, module) => {
	module.exports = { "x": true };
}));

//#endregion
//#region entry.js
const x_json = require_x();
console.log(x_json, y_default, small, _if);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,15 +1,13 @@
-var require_x = __commonJS({
-    "x.json"(exports, module) {
-        module.exports = {
-            x: true
-        };
-    }
-});
 var y_default = {
     y1: true,
     y2: false
 };
 var small = "some small text";
-var if2 = "test keyword imports";
+var _if = "test keyword imports";
+var require_x = __commonJSMin((exports, module) => {
+    module.exports = {
+        "x": true
+    };
+});
 var x_json = require_x();
-console.log(x_json, y_default, small, if2);
+console.log(x_json, y_default, small, _if);

```