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

//#region node_modules/demo-pkg/module.browser.js
var module_browser_exports = {};
__export(module_browser_exports, { default: () => module_browser_default });
var module_browser_default;
var init_module_browser = __esm({ "node_modules/demo-pkg/module.browser.js"() {
	module_browser_default = "browser module";
} });

//#region src/test-main.js
assert.deepEqual((init_module_browser(), __toCommonJS(module_browser_exports)), { default: "browser main" });

//#region src/test-module.js
init_module_browser();
assert.equal(module_browser_default, "browser module");

```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out.js
+++ rolldown	entry.js
@@ -1,8 +1,39 @@
-var require_main_browser = __commonJS({
-    "Users/user/project/node_modules/demo-pkg/main.browser.js"(exports, module) {
-        module.exports = "browser main";
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
     }
+    return to;
+};
+var __toCommonJS = mod => __copyProps(__defProp({}, "__esModule", {
+    value: true
+}), mod);
+var module_browser_exports = {};
+__export(module_browser_exports, {
+    default: () => module_browser_default
 });
-console.log(require_main_browser());
-var import_demo_pkg = __toESM(require_main_browser());
-console.log(import_demo_pkg.default);
+var module_browser_default;
+var init_module_browser = __esm({
+    "node_modules/demo-pkg/module.browser.js"() {
+        module_browser_default = "browser module";
+    }
+});
+console.log((init_module_browser(), __toCommonJS(module_browser_exports)), module_browser_default);
+init_module_browser();
+console.log((init_module_browser(), __toCommonJS(module_browser_exports)), module_browser_default);

```