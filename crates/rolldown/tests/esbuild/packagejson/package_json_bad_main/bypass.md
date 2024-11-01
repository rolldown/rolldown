# Reason
1. different fs
# Diff
## /Users/user/project/out.js
### esbuild
```js
// Users/user/project/node_modules/demo-pkg/index.js
var require_demo_pkg = __commonJS({
  "Users/user/project/node_modules/demo-pkg/index.js"(exports, module) {
    module.exports = function() {
      return 123;
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


//#region node_modules/demo-pkg/index.js
var require_demo_pkg = __commonJS({ "node_modules/demo-pkg/index.js"(exports, module) {
	module.exports = function() {
		return 123;
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
@@ -1,6 +1,6 @@
 var require_demo_pkg = __commonJS({
-    "Users/user/project/node_modules/demo-pkg/index.js"(exports, module) {
+    "node_modules/demo-pkg/index.js"(exports, module) {
         module.exports = function () {
             return 123;
         };
     }

```