# Diff
## /y-YE5AYNFB.txt
### esbuild
```js
y
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/y-YE5AYNFB.txt
+++ rolldown	
@@ -1,1 +0,0 @@
-y;

```
## /x-LSAMBFUD.txt
### esbuild
```js
x
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/x-LSAMBFUD.txt
+++ rolldown	
@@ -1,1 +0,0 @@
-x;

```
## /out.js
### esbuild
```js
// x.txt
var require_x = __commonJS({
  "x.txt"(exports, module) {
    module.exports = "./x-LSAMBFUD.txt";
  }
});

// y.txt
var y_default = "./y-YE5AYNFB.txt";

// entry.js
var x_url = require_x();
console.log(x_url, y_default);
```
### rolldown
```js


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
-        module.exports = "./x-LSAMBFUD.txt";
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
-var y_default = "./y-YE5AYNFB.txt";
-var x_url = require_x();
+var x_url = (init_x(), __toCommonJS(x_exports));
 console.log(x_url, y_default);

```