# Reason
1. different fs
2. different naming style
# Diff
## /Users/user/project/out.js
### esbuild
```js
// Users/user/project/node_modules/demo-pkg/main.js
var require_main = __commonJS({
  "Users/user/project/node_modules/demo-pkg/main.js"(exports, module) {
    module.exports = function() {
      return 123;
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


//#region node_modules/demo-pkg/main.js
var import_main;
var require_main = __commonJS({ "node_modules/demo-pkg/main.js"(exports, module) {
	module.exports = function() {
		return 123;
	};
	import_main = __toESM(require_main());
} });

//#endregion
//#region src/entry.js
require_main();
assert.equal((0, import_main.default)(), 123);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out.js
+++ rolldown	entry.js
@@ -1,9 +1,11 @@
+var import_main;
 var require_main = __commonJS({
-    "Users/user/project/node_modules/demo-pkg/main.js"(exports, module) {
+    "node_modules/demo-pkg/main.js"(exports, module) {
         module.exports = function () {
             return 123;
         };
+        import_main = __toESM(require_main());
     }
 });
-var import_demo_pkg = __toESM(require_main());
-console.log((0, import_demo_pkg.default)());
+require_main();
+console.log((0, import_main.default)());

```