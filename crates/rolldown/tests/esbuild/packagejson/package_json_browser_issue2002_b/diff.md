# Reason
1. `sub` is not resolved
# Diff
## /Users/user/project/out.js
### esbuild
```js
// Users/user/project/src/node_modules/pkg/sub/bar.js
var require_bar = __commonJS({
  "Users/user/project/src/node_modules/pkg/sub/bar.js"() {
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
//#region src/node_modules/pkg/sub/foo.js
var require_foo = /* @__PURE__ */ __commonJS({ "src/node_modules/pkg/sub/foo.js"() {
	__require("sub");
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
@@ -1,11 +1,6 @@
-var require_bar = __commonJS({
-    "Users/user/project/src/node_modules/pkg/sub/bar.js"() {
-        works();
-    }
-});
 var require_foo = __commonJS({
-    "Users/user/project/src/node_modules/pkg/sub/foo.js"() {
-        require_bar();
+    "src/node_modules/pkg/sub/foo.js"() {
+        __require("sub");
     }
 });
 require_foo();

```