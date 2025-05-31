# Diff
## /Users/user/project/out.js
### esbuild
```js
// Users/user/project/node_modules/demo-pkg/main.browser.js
var require_main_browser = __commonJS({
  "Users/user/project/node_modules/demo-pkg/main.browser.js"(exports, module) {
    module.exports = "browser main";
  }
});

// Users/user/project/src/test-main.js
console.log(require_main_browser());

// Users/user/project/src/test-module.js
var import_demo_pkg = __toESM(require_main_browser());
console.log(import_demo_pkg.default);
```
### rolldown
```js
import assert from "node:assert";


//#region node_modules/demo-pkg/module.browser.js
var module_browser_exports = {};
__export(module_browser_exports, { default: () => module_browser_default });
var module_browser_default = "browser module";
var init_module_browser = __esm({ "node_modules/demo-pkg/module.browser.js"() {} });

//#endregion
//#region src/test-main.js
assert.deepEqual((init_module_browser(), __toCommonJS(module_browser_exports)), { default: "browser main" });

//#endregion
//#region src/test-module.js
init_module_browser();
assert.equal(module_browser_default, "browser module");

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out.js
+++ rolldown	entry.js
@@ -1,8 +1,11 @@
-var require_main_browser = __commonJS({
-    "Users/user/project/node_modules/demo-pkg/main.browser.js"(exports, module) {
-        module.exports = "browser main";
-    }
+var module_browser_exports = {};
+__export(module_browser_exports, {
+    default: () => module_browser_default
 });
-console.log(require_main_browser());
-var import_demo_pkg = __toESM(require_main_browser());
-console.log(import_demo_pkg.default);
+var module_browser_default = "browser module";
+var init_module_browser = __esm({
+    "node_modules/demo-pkg/module.browser.js"() {}
+});
+console.log((init_module_browser(), __toCommonJS(module_browser_exports)), module_browser_default);
+init_module_browser();
+console.log((init_module_browser(), __toCommonJS(module_browser_exports)), module_browser_default);

```