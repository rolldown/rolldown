# Diff
## /Users/user/project/out.js
### esbuild
```js
// (disabled):fs
var require_fs = __commonJS({
  "(disabled):fs"() {
  }
});

// Users/user/project/node_modules/demo-pkg/index.js
var require_demo_pkg = __commonJS({
  "Users/user/project/node_modules/demo-pkg/index.js"(exports, module) {
    var fs = require_fs();
    module.exports = function() {
      return fs.readFile();
    };
  }
});

// Users/user/project/src/entry.js
var import_demo_pkg = __toESM(require_demo_pkg());
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
@@ -1,13 +0,0 @@
-var require_fs = __commonJS({
-    "(disabled):fs"() {}
-});
-var require_demo_pkg = __commonJS({
-    "Users/user/project/node_modules/demo-pkg/index.js"(exports, module) {
-        var fs = require_fs();
-        module.exports = function () {
-            return fs.readFile();
-        };
-    }
-});
-var import_demo_pkg = __toESM(require_demo_pkg());
-console.log((0, import_demo_pkg.default)());

```