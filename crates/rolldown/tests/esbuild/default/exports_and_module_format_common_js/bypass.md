# Reason
1. different deconflict naming style and order
# Diff
## /out.js
### esbuild
```js
// foo/test.js
var test_exports = {};
__export(test_exports, {
  foo: () => foo
});
var foo = 123;

// bar/test.js
var test_exports2 = {};
__export(test_exports2, {
  bar: () => bar
});
var bar = 123;

// entry.js
console.log(exports, module.exports, test_exports, test_exports2);
```
### rolldown
```js
//#region rolldown:runtime
var __create = Object.create;
var __defProp = Object.defineProperty;
var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
var __getOwnPropNames = Object.getOwnPropertyNames;
var __getProtoOf = Object.getPrototypeOf;
var __hasOwnProp = Object.prototype.hasOwnProperty;
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
var __toESM = (mod, isNodeMode, target) => (target = mod != null ? __create(__getProtoOf(mod)) : {}, __copyProps(isNodeMode || !mod || !mod.__esModule ? __defProp(target, "default", {
	value: mod,
	enumerable: true
}) : target, mod));

const node_assert = __toESM(require("node:assert"));

//#region foo/test.js
var test_exports$1 = {};
__export(test_exports$1, { foo: () => foo });
let foo = 123;

//#region bar/test.js
var test_exports = {};
__export(test_exports, { bar: () => bar });
let bar = 123;

//#region entry.js
console.log(exports, module.exports);
node_assert.default.deepEqual(test_exports$1, { foo: 123 });
node_assert.default.deepEqual(test_exports, { bar: 123 });

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,11 +1,44 @@
-var test_exports = {};
-__export(test_exports, {
+var __create = Object.create;
+var __defProp = Object.defineProperty;
+var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
+var __getOwnPropNames = Object.getOwnPropertyNames;
+var __getProtoOf = Object.getPrototypeOf;
+var __hasOwnProp = Object.prototype.hasOwnProperty;
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
+var __toESM = (mod, isNodeMode, target) => (target = mod != null ? __create(__getProtoOf(mod)) : {}, __copyProps(isNodeMode || !mod || !mod.__esModule ? __defProp(target, "default", {
+    value: mod,
+    enumerable: true
+}) : target, mod));
+var node_assert = __toESM(require("node:assert"));
+var test_exports$1 = {};
+__export(test_exports$1, {
     foo: () => foo
 });
 var foo = 123;
-var test_exports2 = {};
-__export(test_exports2, {
+var test_exports = {};
+__export(test_exports, {
     bar: () => bar
 });
 var bar = 123;
-console.log(exports, module.exports, test_exports, test_exports2);
+console.log(exports, module.exports);
+node_assert.default.deepEqual(test_exports$1, {
+    foo: 123
+});
+node_assert.default.deepEqual(test_exports, {
+    bar: 123
+});

```