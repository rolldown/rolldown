# Diff
## /Users/user/project/out.js
### esbuild
```js
// Users/user/project/node_modules/demo-pkg/index.js
var require_demo_pkg = __commonJS({
  "Users/user/project/node_modules/demo-pkg/index.js"(exports, module) {
    module.exports = "index";
  }
});

// Users/user/project/src/test-index.js
console.log(require_demo_pkg());

// Users/user/project/src/test-module.js
var import_demo_pkg = __toESM(require_demo_pkg());
console.log(import_demo_pkg.default);
```
### rolldown
```js
import assert, { deepEqual } from "node:assert";


//#region node_modules/demo-pkg/module.js
var module_exports = {};
__export(module_exports, { default: () => module_default });
var module_default = "module";
var init_module = __esm({ "node_modules/demo-pkg/module.js"() {} });

//#endregion
//#region src/test-index.js
deepEqual((init_module(), __toCommonJS(module_exports)), { default: "module" });

//#endregion
//#region src/test-module.js
init_module();
assert.equal(module_default, "module");

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out.js
+++ rolldown	entry.js
@@ -1,8 +1,13 @@
-var require_demo_pkg = __commonJS({
-    "Users/user/project/node_modules/demo-pkg/index.js"(exports, module) {
-        module.exports = "index";
-    }
+var module_exports = {};
+__export(module_exports, {
+    default: () => module_default
 });
-console.log(require_demo_pkg());
-var import_demo_pkg = __toESM(require_demo_pkg());
-console.log(import_demo_pkg.default);
+var module_default = "module";
+var init_module = __esm({
+    "node_modules/demo-pkg/module.js"() {}
+});
+deepEqual((init_module(), __toCommonJS(module_exports)), {
+    default: "module"
+});
+init_module();
+console.log(module_default);

```