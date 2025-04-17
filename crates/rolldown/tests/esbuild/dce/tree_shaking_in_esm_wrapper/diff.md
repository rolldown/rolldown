# Diff
## /out.js
### esbuild
```js
// lib.js
var keep1, keep2;
var init_lib = __esm({
  "lib.js"() {
    keep1 = () => "keep1";
    keep2 = () => "keep2";
  }
});

// cjs.js
var cjs_exports = {};
__export(cjs_exports, {
  default: () => cjs_default
});
var cjs_default;
var init_cjs = __esm({
  "cjs.js"() {
    init_lib();
    cjs_default = keep2();
  }
});

// entry.js
init_lib();
console.log(keep1(), (init_cjs(), __toCommonJS(cjs_exports)));
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

//#region lib.js
var keep1, keep2;
var init_lib = __esm({ "lib.js"() {
	keep1 = () => "keep1";
	keep2 = () => "keep2";
} });

//#region cjs.js
var cjs_exports = {};
__export(cjs_exports, { default: () => cjs_default });
var cjs_default;
var init_cjs = __esm({ "cjs.js"() {
	init_lib();
	cjs_default = keep2();
} });

//#region entry.js
init_lib();
assert.equal(keep1(), "keep1");
assert.deepEqual((init_cjs(), __toCommonJS(cjs_exports)), { default: "keep2" });

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,4 +1,30 @@
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
 var keep1, keep2;
 var init_lib = __esm({
     "lib.js"() {
         keep1 = () => "keep1";

```