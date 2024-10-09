# Diff
## /Users/user/project/out.js
### esbuild
```js
// Users/user/project/node_modules/demo-pkg/index.js
var require_demo_pkg = __commonJS({
  "Users/user/project/node_modules/demo-pkg/index.js"(exports, module) {
    module.exports = "index";
  }
});

// Users/user/project/src/test-index.js
console.log(require_demo_pkg());

// Users/user/project/src/test-module.js
var import_demo_pkg = __toESM(require_demo_pkg());
console.log(import_demo_pkg.default);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out.js
+++ rolldown	
@@ -1,8 +0,0 @@
-var require_demo_pkg = __commonJS({
-    "Users/user/project/node_modules/demo-pkg/index.js"(exports, module) {
-        module.exports = "index";
-    }
-});
-console.log(require_demo_pkg());
-var import_demo_pkg = __toESM(require_demo_pkg());
-console.log(import_demo_pkg.default);

```