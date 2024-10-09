# Diff
## /Users/user/project/out.js
### esbuild
```js
// (disabled):Users/user/project/node_modules/node-pkg/index.js
var require_node_pkg = __commonJS({
  "(disabled):Users/user/project/node_modules/node-pkg/index.js"() {
  }
});

// Users/user/project/node_modules/demo-pkg/index.js
var require_demo_pkg = __commonJS({
  "Users/user/project/node_modules/demo-pkg/index.js"(exports, module) {
    var fn2 = require_node_pkg();
    module.exports = function() {
      return fn2();
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
-var require_node_pkg = __commonJS({
-    "(disabled):Users/user/project/node_modules/node-pkg/index.js"() {}
-});
-var require_demo_pkg = __commonJS({
-    "Users/user/project/node_modules/demo-pkg/index.js"(exports, module) {
-        var fn2 = require_node_pkg();
-        module.exports = function () {
-            return fn2();
-        };
-    }
-});
-var import_demo_pkg = __toESM(require_demo_pkg());
-console.log((0, import_demo_pkg.default)());

```