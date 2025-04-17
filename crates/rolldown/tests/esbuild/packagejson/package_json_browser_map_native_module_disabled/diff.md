# Diff
## /Users/user/project/out.js
### esbuild
```js
// (disabled):fs
var require_fs = __commonJS({
  "(disabled):fs"() {
  }
});

// Users/user/project/node_modules/demo-pkg/index.js
var require_demo_pkg = __commonJS({
  "Users/user/project/node_modules/demo-pkg/index.js"(exports, module) {
    var fs = require_fs();
    module.exports = function() {
      return fs.readFile();
    };
  }
});

// Users/user/project/src/entry.js
var import_demo_pkg = __toESM(require_demo_pkg());
console.log((0, import_demo_pkg.default)());
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
var __commonJS = (cb, mod) => function() {
	return mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod), mod.exports;
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

//#region (ignored) node_modules/demo-pkg
var require_demo_pkg$1 = __commonJS({ "node_modules/demo-pkg"() {} });

//#region node_modules/demo-pkg/index.js
var require_demo_pkg = __commonJS({ "node_modules/demo-pkg/index.js"(exports, module) {
	const fs = require_demo_pkg$1();
	module.exports = function() {
		return fs.readFile();
	};
} });
var import_demo_pkg = __toESM(require_demo_pkg());

//#region src/entry.js
console.log((0, import_demo_pkg.default)());

```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out.js
+++ rolldown	entry.js
@@ -1,10 +1,35 @@
-var require_fs = __commonJS({
-    "(disabled):fs"() {}
+var __create = Object.create;
+var __defProp = Object.defineProperty;
+var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
+var __getOwnPropNames = Object.getOwnPropertyNames;
+var __getProtoOf = Object.getPrototypeOf;
+var __hasOwnProp = Object.prototype.hasOwnProperty;
+var __commonJS = (cb, mod) => function () {
+    return (mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = {
+        exports: {}
+    }).exports, mod), mod.exports);
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
+var require_demo_pkg$1 = __commonJS({
+    "node_modules/demo-pkg"() {}
 });
 var require_demo_pkg = __commonJS({
-    "Users/user/project/node_modules/demo-pkg/index.js"(exports, module) {
-        var fs = require_fs();
+    "node_modules/demo-pkg/index.js"(exports, module) {
+        const fs = require_demo_pkg$1();
         module.exports = function () {
             return fs.readFile();
         };
     }

```