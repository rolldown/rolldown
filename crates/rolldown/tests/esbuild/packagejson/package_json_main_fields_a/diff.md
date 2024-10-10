# Diff
## /Users/user/project/out.js
### esbuild
```js
// Users/user/project/node_modules/demo-pkg/a.js
var require_a = __commonJS({
  "Users/user/project/node_modules/demo-pkg/a.js"(exports, module) {
    module.exports = "a";
  }
});

// Users/user/project/src/entry.js
var import_demo_pkg = __toESM(require_a());
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
@@ -1,7 +0,0 @@
-var require_a = __commonJS({
-    "Users/user/project/node_modules/demo-pkg/a.js"(exports, module) {
-        module.exports = "a";
-    }
-});
-var import_demo_pkg = __toESM(require_a());
-console.log(import_demo_pkg.default);

```