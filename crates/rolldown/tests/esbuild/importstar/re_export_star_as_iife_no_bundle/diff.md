# Reason
1. different iife impl
# Diff
## /out.js
### esbuild
```js
var mod = (() => {
  var entry_exports = {};
  __export(entry_exports, {
    out: () => out
  });
  var out = __toESM(require("foo"));
  return __toCommonJS(entry_exports);
})();
```
### rolldown
```js
var mod = (function(exports, foo) {

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

foo = __toESM(foo);

Object.defineProperty(exports, 'out', {
  enumerable: true,
  get: function () {
    return foo;
  }
});
return exports;
})({}, foo);
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,8 +1,30 @@
-var mod = (() => {
-    var entry_exports = {};
-    __export(entry_exports, {
-        out: () => out
+var mod = (function (exports, foo) {
+    var __create = Object.create;
+    var __defProp = Object.defineProperty;
+    var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
+    var __getOwnPropNames = Object.getOwnPropertyNames;
+    var __getProtoOf = Object.getPrototypeOf;
+    var __hasOwnProp = Object.prototype.hasOwnProperty;
+    var __copyProps = (to, from, except, desc) => {
+        if (from && typeof from === "object" || typeof from === "function") for (var keys = __getOwnPropNames(from), i = 0, n = keys.length, key; i < n; i++) {
+            key = keys[i];
+            if (!__hasOwnProp.call(to, key) && key !== except) __defProp(to, key, {
+                get: (k => from[k]).bind(null, key),
+                enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable
+            });
+        }
+        return to;
+    };
+    var __toESM = (mod, isNodeMode, target) => (target = mod != null ? __create(__getProtoOf(mod)) : {}, __copyProps(isNodeMode || !mod || !mod.__esModule ? __defProp(target, "default", {
+        value: mod,
+        enumerable: true
+    }) : target, mod));
+    foo = __toESM(foo);
+    Object.defineProperty(exports, 'out', {
+        enumerable: true,
+        get: function () {
+            return foo;
+        }
     });
-    var out = __toESM(require("foo"));
-    return __toCommonJS(entry_exports);
-})();
+    return exports;
+})({}, foo);

```