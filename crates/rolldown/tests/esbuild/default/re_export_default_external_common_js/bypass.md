# Reason
1. cjs module lexer can't recognize esbuild interop pattern
# Diff
## /out.js
### esbuild
```js
// entry.js
var entry_exports = {};
__export(entry_exports, {
  bar: () => import_bar.default,
  foo: () => import_foo.default
});
module.exports = __toCommonJS(entry_exports);
var import_foo = __toESM(require("foo"));

// bar.js
var import_bar = __toESM(require("bar"));
```
### rolldown
```js
"use strict";
//#region rolldown:runtime
var __create = Object.create;
var __defProp = Object.defineProperty;
var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
var __getOwnPropNames = Object.getOwnPropertyNames;
var __getProtoOf = Object.getPrototypeOf;
var __hasOwnProp = Object.prototype.hasOwnProperty;
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

const foo = __toESM(require("foo"));
const bar = __toESM(require("bar"));

Object.defineProperty(exports, 'bar', {
  enumerable: true,
  get: function () {
    return bar.default;
  }
});
Object.defineProperty(exports, 'foo', {
  enumerable: true,
  get: function () {
    return foo.default;
  }
});
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,8 +1,34 @@
-var entry_exports = {};
-__export(entry_exports, {
-    bar: () => import_bar.default,
-    foo: () => import_foo.default
+var __create = Object.create;
+var __defProp = Object.defineProperty;
+var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
+var __getOwnPropNames = Object.getOwnPropertyNames;
+var __getProtoOf = Object.getPrototypeOf;
+var __hasOwnProp = Object.prototype.hasOwnProperty;
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
+var foo = __toESM(require("foo"));
+var bar = __toESM(require("bar"));
+Object.defineProperty(exports, 'bar', {
+    enumerable: true,
+    get: function () {
+        return bar.default;
+    }
 });
-module.exports = __toCommonJS(entry_exports);
-var import_foo = __toESM(require("foo"));
-var import_bar = __toESM(require("bar"));
+Object.defineProperty(exports, 'foo', {
+    enumerable: true,
+    get: function () {
+        return foo.default;
+    }
+});

```