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

// Users/user/project/src/test-main.js
console.log((init_module(), __toCommonJS(module_exports)));

// Users/user/project/src/test-module.js
init_module();
console.log(module_default);
```
### rolldown
```js
import assert from "node:assert";

//#region rolldown:runtime
var __defProp = Object.defineProperty;
var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
var __getOwnPropNames = Object.getOwnPropertyNames;
var __hasOwnProp = Object.prototype.hasOwnProperty;
var __esm = (fn, res) => function() {
	return fn && (res = (0, fn[__getOwnPropNames(fn)[0]])(fn = 0)), res;
};
var __export = (target, all) => {
	for (var name in all) __defProp(target, name, {
		get: all[name],
		enumerable: true
	});
};
var __copyProps = (to, from, except, desc) => {
	if (from && typeof from === "object" || typeof from === "function") for (var keys = __getOwnPropNames(from), i = 0, n = keys.length, key; i < n; i++) {
		key = keys[i];
		if (!__hasOwnProp.call(to, key) && key !== except) __defProp(to, key, {
			get: ((k) => from[k]).bind(null, key),
			enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable
		});
	}
	return to;
};
var __toCommonJS = (mod) => __copyProps(__defProp({}, "__esModule", { value: true }), mod);

//#region node_modules/demo-pkg/module.js
var module_exports = {};
__export(module_exports, { default: () => module_default });
var module_default;
var init_module = __esm({ "node_modules/demo-pkg/module.js"() {
	module_default = "module";
} });

//#region src/test-main.js
assert.deepEqual((init_module(), __toCommonJS(module_exports)), { default: "module" });

//#region src/test-module.js
init_module();
assert.equal(module_default, "module");

```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out.js
+++ rolldown	entry.js
@@ -1,13 +1,39 @@
+var __defProp = Object.defineProperty;
+var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
+var __getOwnPropNames = Object.getOwnPropertyNames;
+var __hasOwnProp = Object.prototype.hasOwnProperty;
+var __esm = (fn, res) => function () {
+    return (fn && (res = (0, fn[__getOwnPropNames(fn)[0]])(fn = 0)), res);
+};
+var __export = (target, all) => {
+    for (var name in all) __defProp(target, name, {
+        get: all[name],
+        enumerable: true
+    });
+};
+var __copyProps = (to, from, except, desc) => {
+    if (from && typeof from === "object" || typeof from === "function") for (var keys = __getOwnPropNames(from), i = 0, n = keys.length, key; i < n; i++) {
+        key = keys[i];
+        if (!__hasOwnProp.call(to, key) && key !== except) __defProp(to, key, {
+            get: (k => from[k]).bind(null, key),
+            enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable
+        });
+    }
+    return to;
+};
+var __toCommonJS = mod => __copyProps(__defProp({}, "__esModule", {
+    value: true
+}), mod);
 var module_exports = {};
 __export(module_exports, {
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