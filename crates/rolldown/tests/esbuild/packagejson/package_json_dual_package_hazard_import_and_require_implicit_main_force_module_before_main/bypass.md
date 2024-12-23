# Reason
1. different fs
# Diff
## /Users/user/project/out.js
### esbuild
```js
// Users/user/project/node_modules/demo-pkg/module.js
var module_exports = {};
__export(module_exports, {
  default: () => module_default
});
var module_default;
var init_module = __esm({
  "Users/user/project/node_modules/demo-pkg/module.js"() {
    module_default = "module";
  }
});

// Users/user/project/src/test-index.js
console.log((init_module(), __toCommonJS(module_exports)));

// Users/user/project/src/test-module.js
init_module();
console.log(module_default);
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
//#region src/test-index.js
assert.deepEqual((init_module(), __toCommonJS(module_exports)), { default: "module" });

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
@@ -3,11 +3,11 @@
     default: () => module_default
 });
 var module_default;
 var init_module = __esm({
-    "Users/user/project/node_modules/demo-pkg/module.js"() {
+    "node_modules/demo-pkg/module.js"() {
         module_default = "module";
     }
 });
-console.log((init_module(), __toCommonJS(module_exports)));
+console.log((init_module(), __toCommonJS(module_exports)), module_default);
 init_module();
-console.log(module_default);
+console.log((init_module(), __toCommonJS(module_exports)), module_default);

```