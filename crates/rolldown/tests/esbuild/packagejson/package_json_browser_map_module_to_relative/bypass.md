# Reason
1. different fs
# Diff
## /Users/user/project/out.js
### esbuild
```js
// Users/user/project/node_modules/demo-pkg/node-pkg-browser.js
var require_node_pkg_browser = __commonJS({
  "Users/user/project/node_modules/demo-pkg/node-pkg-browser.js"(exports, module) {
    module.exports = function() {
      return 123;
    };
  }
});

// Users/user/project/node_modules/demo-pkg/index.js
var require_demo_pkg = __commonJS({
  "Users/user/project/node_modules/demo-pkg/index.js"(exports, module) {
    var fn2 = require_node_pkg_browser();
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

// HIDDEN [rolldown:runtime]
//#region node_modules/demo-pkg/node-pkg-browser.js
var require_node_pkg_browser = /* @__PURE__ */ __commonJS({ "node_modules/demo-pkg/node-pkg-browser.js"(exports, module) {
	module.exports = function() {
		return 123;
	};
} });

//#endregion
//#region node_modules/demo-pkg/index.js
var require_demo_pkg = /* @__PURE__ */ __commonJS({ "node_modules/demo-pkg/index.js"(exports, module) {
	const fn$1 = require_node_pkg_browser();
	module.exports = function() {
		return fn$1();
	};
} });

//#endregion
//#region src/entry.js
var import_demo_pkg = __toESM(require_demo_pkg());
assert.equal((0, import_demo_pkg.default)(), 123);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out.js
+++ rolldown	entry.js
@@ -1,16 +1,16 @@
 var require_node_pkg_browser = __commonJS({
-    "Users/user/project/node_modules/demo-pkg/node-pkg-browser.js"(exports, module) {
+    "node_modules/demo-pkg/node-pkg-browser.js"(exports, module) {
         module.exports = function () {
             return 123;
         };
     }
 });
 var require_demo_pkg = __commonJS({
-    "Users/user/project/node_modules/demo-pkg/index.js"(exports, module) {
-        var fn2 = require_node_pkg_browser();
+    "node_modules/demo-pkg/index.js"(exports, module) {
+        const fn$1 = require_node_pkg_browser();
         module.exports = function () {
-            return fn2();
+            return fn$1();
         };
     }
 });
 var import_demo_pkg = __toESM(require_demo_pkg());

```