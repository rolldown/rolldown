# Diff
## /Users/user/project/out.js
### esbuild
```js
// (disabled):Users/user/project/node_modules/demo-pkg/util-node
var require_util_node = __commonJS({
  "(disabled):Users/user/project/node_modules/demo-pkg/util-node"() {
  }
});

// Users/user/project/node_modules/demo-pkg/main.js
var require_main = __commonJS({
  "Users/user/project/node_modules/demo-pkg/main.js"(exports, module) {
    var util = require_util_node();
    module.exports = function(obj) {
      return util.inspect(obj);
    };
  }
});

// Users/user/project/src/entry.js
var import_demo_pkg = __toESM(require_main());
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
-var require_util_node = __commonJS({
-    "(disabled):Users/user/project/node_modules/demo-pkg/util-node"() {}
-});
-var require_main = __commonJS({
-    "Users/user/project/node_modules/demo-pkg/main.js"(exports, module) {
-        var util = require_util_node();
-        module.exports = function (obj) {
-            return util.inspect(obj);
-        };
-    }
-});
-var import_demo_pkg = __toESM(require_main());
-console.log((0, import_demo_pkg.default)());

```