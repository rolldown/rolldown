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

// Users/user/project/src/entry.js
var import_demo_pkg = __toESM(require_main());
console.log(import_demo_pkg.default, require_main());
```
### rolldown
```js
// HIDDEN [rolldown:runtime]
//#region node_modules/demo-pkg/module.js
var module_exports = {};
__export(module_exports, { default: () => module_default });
var module_default;
var init_module = __esm({ "node_modules/demo-pkg/module.js"() {
	module_default = "module";
} });

//#endregion
//#region src/entry.js
init_module();
console.log(module_default, (init_module(), __toCommonJS(module_exports)));

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out.js
+++ rolldown	entry.js
@@ -1,7 +1,12 @@
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
-var import_demo_pkg = __toESM(require_main());
-console.log(import_demo_pkg.default, require_main());
+init_module();
+console.log(module_default, (init_module(), __toCommonJS(module_exports)));

```