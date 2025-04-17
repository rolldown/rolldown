# Reason
1. cjs module lexer can't recognize esbuild interop pattern
# Diff
## /out/entry.js
### esbuild
```js
// entry.js
var entry_exports = {};
__export(entry_exports, {
  inner: () => inner_exports
});
module.exports = __toCommonJS(entry_exports);
__reExport(entry_exports, require("a"), module.exports);

// inner.js
var inner_exports = {};
__reExport(inner_exports, require("b"));
```
### rolldown
```js
"use strict";
//#region rolldown:runtime
var __defProp = Object.defineProperty;
var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
var __getOwnPropNames = Object.getOwnPropertyNames;
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
var __reExport = (target, mod, secondTarget) => (__copyProps(target, mod, "default"), secondTarget && __copyProps(secondTarget, mod, "default"));


//#region inner.js
var inner_exports = {};
__reExport(inner_exports, require("b"));

Object.defineProperty(exports, 'inner', {
  enumerable: true,
  get: function () {
    return inner_exports;
  }
});
var a = require("a");
Object.keys(a).forEach(function (k) {
  if (k !== 'default' && !Object.prototype.hasOwnProperty.call(exports, k)) Object.defineProperty(exports, k, {
    enumerable: true,
    get: function () { return a[k]; }
  });
});

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,8 +1,32 @@
-var entry_exports = {};
-__export(entry_exports, {
-    inner: () => inner_exports
-});
-module.exports = __toCommonJS(entry_exports);
-__reExport(entry_exports, require("a"), module.exports);
+var __defProp = Object.defineProperty;
+var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
+var __getOwnPropNames = Object.getOwnPropertyNames;
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
+var __reExport = (target, mod, secondTarget) => (__copyProps(target, mod, "default"), secondTarget && __copyProps(secondTarget, mod, "default"));
 var inner_exports = {};
 __reExport(inner_exports, require("b"));
+Object.defineProperty(exports, 'inner', {
+    enumerable: true,
+    get: function () {
+        return inner_exports;
+    }
+});
+var a = require("a");
+Object.keys(a).forEach(function (k) {
+    if (k !== 'default' && !Object.prototype.hasOwnProperty.call(exports, k)) Object.defineProperty(exports, k, {
+        enumerable: true,
+        get: function () {
+            return a[k];
+        }
+    });
+});

```