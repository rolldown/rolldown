# Reason
1. different fs 
2. different naming style
# Diff
## /Users/user/project/out.js
### esbuild
```js
// Users/user/project/node_modules/demo-pkg/a.js
var require_a = __commonJS({
  "Users/user/project/node_modules/demo-pkg/a.js"(exports, module) {
    module.exports = "a";
  }
});

// Users/user/project/src/entry.js
var import_demo_pkg = __toESM(require_a());
console.log(import_demo_pkg.default);
```
### rolldown
```js
import assert from "node:assert";

// HIDDEN [rolldown:runtime]
//#region node_modules/demo-pkg/a.js
var require_a = /* @__PURE__ */ __commonJS({ "node_modules/demo-pkg/a.js"(exports, module) {
	module.exports = "a";
} });

//#endregion
//#region src/entry.js
var import_a = __toESM(require_a());
assert.equal(import_a.default, "a");

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out.js
+++ rolldown	entry.js
@@ -1,7 +1,7 @@
 var require_a = __commonJS({
-    "Users/user/project/node_modules/demo-pkg/a.js"(exports, module) {
+    "node_modules/demo-pkg/a.js"(exports, module) {
         module.exports = "a";
     }
 });
-var import_demo_pkg = __toESM(require_a());
-console.log(import_demo_pkg.default);
+var import_a = __toESM(require_a());
+console.log(import_a.default);

```