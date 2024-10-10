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

```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out.js
+++ rolldown	
@@ -1,13 +0,0 @@
-var module_exports = {};
-__export(module_exports, {
-    default: () => module_default
-});
-var module_default;
-var init_module = __esm({
-    "Users/user/project/node_modules/demo-pkg/module.js"() {
-        module_default = "module";
-    }
-});
-console.log((init_module(), __toCommonJS(module_exports)));
-init_module();
-console.log(module_default);

```