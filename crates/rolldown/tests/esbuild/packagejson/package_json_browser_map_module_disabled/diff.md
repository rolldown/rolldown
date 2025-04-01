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
import assert from "node:assert";



//#region (ignored) node_modules/demo-pkg
var require_demo_pkg$1 = __commonJS({ "node_modules/demo-pkg"() {} });
//#endregion

//#region node_modules/demo-pkg/index.js
var require_demo_pkg = __commonJS({ "node_modules/demo-pkg/index.js"(exports, module) {
	const fn = require_demo_pkg$1();
	module.exports = function() {
		return fn();
	};
} });
var import_demo_pkg = __toESM(require_demo_pkg());
//#endregion

//#region src/entry.js
assert.equal((0, import_demo_pkg.default)(), 234);
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out.js
+++ rolldown	entry.js
@@ -1,12 +1,12 @@
-var require_node_pkg = __commonJS({
-    "(disabled):Users/user/project/node_modules/node-pkg/index.js"() {}
+var require_demo_pkg$1 = __commonJS({
+    "node_modules/demo-pkg"() {}
 });
 var require_demo_pkg = __commonJS({
-    "Users/user/project/node_modules/demo-pkg/index.js"(exports, module) {
-        var fn2 = require_node_pkg();
+    "node_modules/demo-pkg/index.js"(exports, module) {
+        const fn = require_demo_pkg$1();
         module.exports = function () {
-            return fn2();
+            return fn();
         };
     }
 });
 var import_demo_pkg = __toESM(require_demo_pkg());

```