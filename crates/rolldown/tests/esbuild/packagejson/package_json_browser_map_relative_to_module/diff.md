# Diff
## /Users/user/project/out.js
### esbuild
```js
// Users/user/project/node_modules/util-browser/index.js
var require_util_browser = __commonJS({
  "Users/user/project/node_modules/util-browser/index.js"(exports, module) {
    module.exports = "util-browser";
  }
});

// Users/user/project/node_modules/demo-pkg/main.js
var require_main = __commonJS({
  "Users/user/project/node_modules/demo-pkg/main.js"(exports, module) {
    var util = require_util_browser();
    module.exports = function() {
      return ["main", util];
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
@@ -1,15 +0,0 @@
-var require_util_browser = __commonJS({
-    "Users/user/project/node_modules/util-browser/index.js"(exports, module) {
-        module.exports = "util-browser";
-    }
-});
-var require_main = __commonJS({
-    "Users/user/project/node_modules/demo-pkg/main.js"(exports, module) {
-        var util = require_util_browser();
-        module.exports = function () {
-            return ["main", util];
-        };
-    }
-});
-var import_demo_pkg = __toESM(require_main());
-console.log((0, import_demo_pkg.default)());

```