# Reason
1. different fs
2. different naming style
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
import assert from "node:assert";

// HIDDEN [rolldown:runtime]
//#region (ignored) node_modules/demo-pkg/util-node.js
var require_util_node = __commonJS({ "node_modules/demo-pkg/util-node.js"() {} });

//#endregion
//#region node_modules/demo-pkg/main.js
var require_main = __commonJS({ "node_modules/demo-pkg/main.js"(exports, module) {
	const util = require_util_node();
	module.exports = function(obj) {
		return util.inspect(obj);
	};
} });

//#endregion
//#region src/entry.js
var import_main = __toESM(require_main());
assert.deepEqual((0, import_main.default)(), {});

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out.js
+++ rolldown	entry.js
@@ -1,13 +1,13 @@
 var require_util_node = __commonJS({
-    "(disabled):Users/user/project/node_modules/demo-pkg/util-node"() {}
+    "node_modules/demo-pkg/util-node.js"() {}
 });
 var require_main = __commonJS({
-    "Users/user/project/node_modules/demo-pkg/main.js"(exports, module) {
-        var util = require_util_node();
+    "node_modules/demo-pkg/main.js"(exports, module) {
+        const util = require_util_node();
         module.exports = function (obj) {
             return util.inspect(obj);
         };
     }
 });
-var import_demo_pkg = __toESM(require_main());
-console.log((0, import_demo_pkg.default)());
+var import_main = __toESM(require_main());
+console.log((0, import_main.default)());

```