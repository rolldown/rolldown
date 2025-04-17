# Reason
1. different fs
2. different naming style
# Diff
## /Users/user/project/out.js
### esbuild
```js
// Users/user/project/node_modules/demo-pkg/lib/util-browser.js
var require_util_browser = __commonJS({
  "Users/user/project/node_modules/demo-pkg/lib/util-browser.js"(exports, module) {
    module.exports = "util-browser";
  }
});

// Users/user/project/node_modules/demo-pkg/main-browser.js
var require_main_browser = __commonJS({
  "Users/user/project/node_modules/demo-pkg/main-browser.js"(exports, module) {
    var util = require_util_browser();
    module.exports = function() {
      return ["main-browser", util];
    };
  }
});

// Users/user/project/src/entry.js
var import_demo_pkg = __toESM(require_main_browser());
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

//#region node_modules/demo-pkg/lib/util-browser.js
var require_util_browser = __commonJS({ "node_modules/demo-pkg/lib/util-browser.js"(exports, module) {
	module.exports = "util-browser";
} });

//#region node_modules/demo-pkg/main-browser.js
var require_main_browser = __commonJS({ "node_modules/demo-pkg/main-browser.js"(exports, module) {
	const util = require_util_browser();
	module.exports = function() {
		return ["main-browser", util];
	};
} });
var import_main_browser = __toESM(require_main_browser());

//#region src/entry.js
assert.deepEqual((0, import_main_browser.default)(), ["main-browser", "util-browser"]);

```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out.js
+++ rolldown	entry.js
@@ -1,15 +1,40 @@
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
 var require_util_browser = __commonJS({
-    "Users/user/project/node_modules/demo-pkg/lib/util-browser.js"(exports, module) {
+    "node_modules/demo-pkg/lib/util-browser.js"(exports, module) {
         module.exports = "util-browser";
     }
 });
 var require_main_browser = __commonJS({
-    "Users/user/project/node_modules/demo-pkg/main-browser.js"(exports, module) {
-        var util = require_util_browser();
+    "node_modules/demo-pkg/main-browser.js"(exports, module) {
+        const util = require_util_browser();
         module.exports = function () {
             return ["main-browser", util];
         };
     }
 });
-var import_demo_pkg = __toESM(require_main_browser());
-console.log((0, import_demo_pkg.default)());
+var import_main_browser = __toESM(require_main_browser());
+console.log((0, import_main_browser.default)());

```