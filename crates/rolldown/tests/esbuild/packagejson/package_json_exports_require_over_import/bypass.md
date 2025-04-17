# Reason
1. different fs
# Diff
## /Users/user/project/out.js
### esbuild
```js
// Users/user/project/node_modules/pkg/require.js
var require_require = __commonJS({
  "Users/user/project/node_modules/pkg/require.js"() {
    console.log("SUCCESS");
  }
});

// Users/user/project/src/entry.js
require_require();
```
### rolldown
```js

//#region rolldown:runtime
var __getOwnPropNames = Object.getOwnPropertyNames;
var __commonJS = (cb, mod) => function() {
	return mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod), mod.exports;
};

//#region node_modules/pkg/require.js
var require_require = __commonJS({ "node_modules/pkg/require.js"() {
	console.log("SUCCESS");
} });

//#region src/entry.js
require_require();

```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out.js
+++ rolldown	entry.js
@@ -1,6 +1,12 @@
+var __getOwnPropNames = Object.getOwnPropertyNames;
+var __commonJS = (cb, mod) => function () {
+    return (mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = {
+        exports: {}
+    }).exports, mod), mod.exports);
+};
 var require_require = __commonJS({
-    "Users/user/project/node_modules/pkg/require.js"() {
+    "node_modules/pkg/require.js"() {
         console.log("SUCCESS");
     }
 });
 require_require();

```