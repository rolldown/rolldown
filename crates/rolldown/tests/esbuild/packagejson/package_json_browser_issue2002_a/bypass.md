# Reason
1. different fs
# Diff
## /Users/user/project/out.js
### esbuild
```js
// Users/user/project/src/node_modules/sub/bar.js
var require_bar = __commonJS({
  "Users/user/project/src/node_modules/sub/bar.js"() {
    works();
  }
});

// Users/user/project/src/node_modules/pkg/sub/foo.js
var require_foo = __commonJS({
  "Users/user/project/src/node_modules/pkg/sub/foo.js"() {
    require_bar();
  }
});

// Users/user/project/src/entry.js
require_foo();
```
### rolldown
```js
// HIDDEN [rolldown:runtime]
//#region src/node_modules/sub/bar.js
var require_bar = /* @__PURE__ */ __commonJS({ "src/node_modules/sub/bar.js"() {
	works();
} });

//#endregion
//#region src/node_modules/pkg/sub/foo.js
var require_foo = /* @__PURE__ */ __commonJS({ "src/node_modules/pkg/sub/foo.js"() {
	require_bar();
} });

//#endregion
//#region src/entry.js
require_foo();

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out.js
+++ rolldown	entry.js
@@ -1,11 +1,11 @@
 var require_bar = __commonJS({
-    "Users/user/project/src/node_modules/sub/bar.js"() {
+    "src/node_modules/sub/bar.js"() {
         works();
     }
 });
 var require_foo = __commonJS({
-    "Users/user/project/src/node_modules/pkg/sub/foo.js"() {
+    "src/node_modules/pkg/sub/foo.js"() {
         require_bar();
     }
 });
 require_foo();

```