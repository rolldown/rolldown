# Diff
## /Users/user/project/out.js
### esbuild
```js
// Users/user/project/node_modules/demo-pkg/main.js
var require_main = __commonJS({
  "Users/user/project/node_modules/demo-pkg/main.js"(exports, module) {
    module.exports = "main";
  }
});

// Users/user/project/src/test-main.js
console.log(require_main());

// Users/user/project/src/test-module.js
var import_demo_pkg = __toESM(require_main());
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
-var require_main = __commonJS({
-    "Users/user/project/node_modules/demo-pkg/main.js"(exports, module) {
-        module.exports = "main";
-    }
-});
-console.log(require_main());
-var import_demo_pkg = __toESM(require_main());
-console.log(import_demo_pkg.default);

```