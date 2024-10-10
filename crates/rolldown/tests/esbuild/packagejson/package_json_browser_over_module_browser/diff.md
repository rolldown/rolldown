# Diff
## /Users/user/project/out.js
### esbuild
```js
// Users/user/project/node_modules/demo-pkg/main.browser.js
var require_main_browser = __commonJS({
  "Users/user/project/node_modules/demo-pkg/main.browser.js"(exports, module) {
    module.exports = function() {
      return 123;
    };
  }
});

// Users/user/project/src/entry.js
var import_demo_pkg = __toESM(require_main_browser());
console.log((0, import_demo_pkg.default)());
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out.js
+++ rolldown	
@@ -1,9 +0,0 @@
-var require_main_browser = __commonJS({
-    "Users/user/project/node_modules/demo-pkg/main.browser.js"(exports, module) {
-        module.exports = function () {
-            return 123;
-        };
-    }
-});
-var import_demo_pkg = __toESM(require_main_browser());
-console.log((0, import_demo_pkg.default)());

```