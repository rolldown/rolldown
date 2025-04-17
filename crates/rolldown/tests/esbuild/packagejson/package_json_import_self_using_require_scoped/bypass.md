# Reason
1. different fs
# Diff
## /Users/user/project/out.js
### esbuild
```js
// Users/user/project/src/foo-require.js
var require_foo_require = __commonJS({
  "Users/user/project/src/foo-require.js"(exports, module) {
    module.exports = "foo";
  }
});

// Users/user/project/src/index.js
var require_src = __commonJS({
  "Users/user/project/src/index.js"(exports, module) {
    module.exports = "index";
    console.log(
      require_src(),
      require_foo_require()
    );
  }
});
export default require_src();
```
### rolldown
```js

//#region rolldown:runtime
var __getOwnPropNames = Object.getOwnPropertyNames;
var __commonJS = (cb, mod) => function() {
	return mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod), mod.exports;
};

//#region src/foo-require.js
var require_foo_require = __commonJS({ "src/foo-require.js"(exports, module) {
	module.exports = "foo";
} });

//#region src/index.js
var require_src = __commonJS({ "src/index.js"(exports, module) {
	module.exports = "index";
	console.log(require_src(), require_foo_require());
} });

export default require_src();

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
 var require_foo_require = __commonJS({
-    "Users/user/project/src/foo-require.js"(exports, module) {
+    "src/foo-require.js"(exports, module) {
         module.exports = "foo";
     }
 });
 var require_src = __commonJS({
-    "Users/user/project/src/index.js"(exports, module) {
+    "src/index.js"(exports, module) {
         module.exports = "index";
         console.log(require_src(), require_foo_require());
     }
 });

```