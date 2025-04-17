# Reason
1. different fs
# Diff
## /Users/user/project/out.js
### esbuild
```js
// Users/user/project/src/node_modules/sub/index.js
var require_sub = __commonJS({
  "Users/user/project/src/node_modules/sub/index.js"() {
    works();
  }
});

// Users/user/project/src/node_modules/pkg/sub/foo.js
var require_foo = __commonJS({
  "Users/user/project/src/node_modules/pkg/sub/foo.js"() {
    require_sub();
  }
});

// Users/user/project/src/entry.js
require_foo();
```
### rolldown
```js

//#region rolldown:runtime
var __getOwnPropNames = Object.getOwnPropertyNames;
var __commonJS = (cb, mod) => function() {
	return mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod), mod.exports;
};

//#region src/node_modules/sub/index.js
var require_sub = __commonJS({ "src/node_modules/sub/index.js"() {
	works();
} });

//#region src/node_modules/pkg/sub/foo.js
var require_foo = __commonJS({ "src/node_modules/pkg/sub/foo.js"() {
	require_sub();
} });

//#region src/entry.js
require_foo();

```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out.js
+++ rolldown	entry.js
@@ -1,11 +1,17 @@
+var __getOwnPropNames = Object.getOwnPropertyNames;
+var __commonJS = (cb, mod) => function () {
+    return (mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = {
+        exports: {}
+    }).exports, mod), mod.exports);
+};
 var require_sub = __commonJS({
-    "Users/user/project/src/node_modules/sub/index.js"() {
+    "src/node_modules/sub/index.js"() {
         works();
     }
 });
 var require_foo = __commonJS({
-    "Users/user/project/src/node_modules/pkg/sub/foo.js"() {
+    "src/node_modules/pkg/sub/foo.js"() {
         require_sub();
     }
 });
 require_foo();

```