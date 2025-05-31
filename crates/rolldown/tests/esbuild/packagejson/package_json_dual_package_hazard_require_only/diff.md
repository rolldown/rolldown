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
console.log(require_main());
```
### rolldown
```js

//#region node_modules/demo-pkg/module.js
var module_exports = {};
__export(module_exports, { default: () => module_default });
var module_default = "module";
var init_module = __esm({ "node_modules/demo-pkg/module.js"() {} });

//#endregion
//#region src/entry.js
console.log((init_module(), __toCommonJS(module_exports)));

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out.js
+++ rolldown	entry.js
@@ -1,6 +1,9 @@
-var require_main = __commonJS({
-    "Users/user/project/node_modules/demo-pkg/main.js"(exports, module) {
-        module.exports = "main";
-    }
+var module_exports = {};
+__export(module_exports, {
+    default: () => module_default
 });
-console.log(require_main());
+var module_default = "module";
+var init_module = __esm({
+    "node_modules/demo-pkg/module.js"() {}
+});
+console.log((init_module(), __toCommonJS(module_exports)));

```