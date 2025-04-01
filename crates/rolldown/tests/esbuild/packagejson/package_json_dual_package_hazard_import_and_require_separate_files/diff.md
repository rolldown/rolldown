# Diff
## /Users/user/project/out.js
### esbuild
```js
// Users/user/project/node_modules/demo-pkg/main.js
var require_main = __commonJS({
  "Users/user/project/node_modules/demo-pkg/main.js"(exports, module) {
    module.exports = "main";
  }
});

// Users/user/project/src/test-main.js
console.log(require_main());

// Users/user/project/src/test-module.js
var import_demo_pkg = __toESM(require_main());
console.log(import_demo_pkg.default);
```
### rolldown
```js
import assert from "node:assert";



//#region node_modules/demo-pkg/module.js
var module_exports = {};
__export(module_exports, { default: () => module_default });
var module_default;
var init_module = __esm({ "node_modules/demo-pkg/module.js"() {
	module_default = "module";
} });
//#endregion

//#region src/test-main.js
console.log((init_module(), __toCommonJS(module_exports)));
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
-var require_main = __commonJS({
-    "Users/user/project/node_modules/demo-pkg/main.js"(exports, module) {
-        module.exports = "main";
+var module_exports = {};
+__export(module_exports, {
+    default: () => module_default
+});
+var module_default;
+var init_module = __esm({
+    "node_modules/demo-pkg/module.js"() {
+        module_default = "module";
     }
 });
-console.log(require_main());
-var import_demo_pkg = __toESM(require_main());
-console.log(import_demo_pkg.default);
+console.log((init_module(), __toCommonJS(module_exports)));
+init_module();
+console.log(module_default);

```