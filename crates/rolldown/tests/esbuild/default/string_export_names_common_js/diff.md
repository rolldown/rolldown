# Reason
1. should not reuse `__toESM(require('./foo'))`
# Diff
## /out.js
### esbuild
```js
var entry_exports = {};
__export(entry_exports, {
  "all the stuff": () => all_the_stuff,
  "some export": () => import_foo["some import"]
});
module.exports = __toCommonJS(entry_exports);
var import_foo = require("./foo");
var all_the_stuff = __toESM(require("./foo"));
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

const foo = __toESM(require("./foo"));

Object.defineProperty(exports, 'all the stuff', {
  enumerable: true,
  get: function () {
    return foo;
  }
});
exports["some export"] = foo["some import"]
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,8 +1,28 @@
-var entry_exports = {};
-__export(entry_exports, {
-    "all the stuff": () => all_the_stuff,
-    "some export": () => import_foo["some import"]
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
+var foo = __toESM(require("./foo"));
+Object.defineProperty(exports, 'all the stuff', {
+    enumerable: true,
+    get: function () {
+        return foo;
+    }
 });
-module.exports = __toCommonJS(entry_exports);
-var import_foo = require("./foo");
-var all_the_stuff = __toESM(require("./foo"));
+exports["some export"] = foo["some import"];

```