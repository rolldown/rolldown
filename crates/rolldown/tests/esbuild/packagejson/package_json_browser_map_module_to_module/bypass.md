# Reason
1. different fs
2. different deconflict naming style
# Diff
## /Users/user/project/out.js
### esbuild
```js
// Users/user/project/node_modules/node-pkg-browser/index.js
var require_node_pkg_browser = __commonJS({
  "Users/user/project/node_modules/node-pkg-browser/index.js"(exports, module) {
    module.exports = function() {
      return 123;
    };
  }
});

// Users/user/project/node_modules/demo-pkg/index.js
var require_demo_pkg = __commonJS({
  "Users/user/project/node_modules/demo-pkg/index.js"(exports, module) {
    var fn2 = require_node_pkg_browser();
    module.exports = function() {
      return fn2();
    };
  }
});

// Users/user/project/src/entry.js
var import_demo_pkg = __toESM(require_demo_pkg());
console.log((0, import_demo_pkg.default)());
```
### rolldown
```js
import assert from "node:assert";

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

//#region node_modules/node-pkg-browser/index.js
var require_node_pkg_browser = __commonJS({ "node_modules/node-pkg-browser/index.js"(exports, module) {
	module.exports = function() {
		return 123;
	};
} });

//#region node_modules/demo-pkg/index.js
var require_demo_pkg = __commonJS({ "node_modules/demo-pkg/index.js"(exports, module) {
	const fn = require_node_pkg_browser();
	module.exports = function() {
		return fn();
	};
} });
var import_demo_pkg = __toESM(require_demo_pkg());

//#region src/entry.js
assert.equal((0, import_demo_pkg.default)(), 123);

```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out.js
+++ rolldown	entry.js
@@ -1,16 +1,41 @@
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
 var require_node_pkg_browser = __commonJS({
-    "Users/user/project/node_modules/node-pkg-browser/index.js"(exports, module) {
+    "node_modules/node-pkg-browser/index.js"(exports, module) {
         module.exports = function () {
             return 123;
         };
     }
 });
 var require_demo_pkg = __commonJS({
-    "Users/user/project/node_modules/demo-pkg/index.js"(exports, module) {
-        var fn2 = require_node_pkg_browser();
+    "node_modules/demo-pkg/index.js"(exports, module) {
+        const fn = require_node_pkg_browser();
         module.exports = function () {
-            return fn2();
+            return fn();
         };
     }
 });
 var import_demo_pkg = __toESM(require_demo_pkg());

```