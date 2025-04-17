# Reason
1. string export name not correct
# Diff
## /out.js
### esbuild
```js
var global;
(global ||= {}).name = (() => {
  var entry_exports = {};
  __export(entry_exports, {
    "all the stuff": () => all_the_stuff,
    "some export": () => import_foo["some import"]
  });
  var import_foo = require("./foo");
  var all_the_stuff = __toESM(require("./foo"));
  return __toCommonJS(entry_exports);
})();
```
### rolldown
```js
(function(exports, foo) {

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

Object.defineProperty(exports, 'all the stuff', {
  enumerable: true,
  get: function () {
    return foo;
  }
});
exports["some export"] = foo["some import"]
return exports;
})({}, foo);
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,11 +1,31 @@
-var global;
-(global ||= {}).name = (() => {
-    var entry_exports = {};
-    __export(entry_exports, {
-        "all the stuff": () => all_the_stuff,
-        "some export": () => import_foo["some import"]
+(function (exports, foo) {
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
+    Object.defineProperty(exports, 'all the stuff', {
+        enumerable: true,
+        get: function () {
+            return foo;
+        }
     });
-    var import_foo = require("./foo");
-    var all_the_stuff = __toESM(require("./foo"));
-    return __toCommonJS(entry_exports);
-})();
+    exports["some export"] = foo["some import"];
+    return exports;
+})({}, foo);

```