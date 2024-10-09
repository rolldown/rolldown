# Diff
## /Users/user/project/out.js
### esbuild
```js
// Users/user/project/node_modules/demo-pkg/main.browser.js
var require_main_browser = __commonJS({
  "Users/user/project/node_modules/demo-pkg/main.browser.js"(exports, module) {
    module.exports = "browser main";
  }
});

// Users/user/project/src/test-main.js
console.log(require_main_browser());

// Users/user/project/src/test-module.js
var import_demo_pkg = __toESM(require_main_browser());
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
-var require_main_browser = __commonJS({
-    "Users/user/project/node_modules/demo-pkg/main.browser.js"(exports, module) {
-        module.exports = "browser main";
-    }
-});
-console.log(require_main_browser());
-var import_demo_pkg = __toESM(require_main_browser());
-console.log(import_demo_pkg.default);

```