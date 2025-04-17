# Diff
## /out.js
### esbuild
```js
// foo.js
var require_foo = __commonJS({
  "foo.js"(exports) {
    exports.foo = function() {
      return "foo";
    };
  }
});

// bar.js
var require_bar = __commonJS({
  "bar.js"(exports) {
    exports.bar = function() {
      return "bar";
    };
  }
});

// entry.js
var import_foo = __toESM(require_foo());
var import_bar = __toESM(require_bar());
console.log((0, import_foo.foo)(), (0, import_bar.bar)());
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

//#region foo.js
var require_foo = __commonJS({ "foo.js"(exports) {
	exports.foo = function() {
		return "foo";
	};
} });
var import_foo = __toESM(require_foo());

//#region bar.js
var require_bar = __commonJS({ "bar.js"(exports) {
	exports.bar = function() {
		return "bar";
	};
} });
var import_bar = __toESM(require_bar());

//#region entry.js
assert.equal((0, import_foo.foo)(), "foo");
assert.equal((0, import_bar.bar)(), "bar");

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,17 +1,42 @@
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
 var require_foo = __commonJS({
     "foo.js"(exports) {
         exports.foo = function () {
             return "foo";
         };
     }
 });
+var import_foo = __toESM(require_foo());
 var require_bar = __commonJS({
     "bar.js"(exports) {
         exports.bar = function () {
             return "bar";
         };
     }
 });
-var import_foo = __toESM(require_foo());
 var import_bar = __toESM(require_bar());
 console.log((0, import_foo.foo)(), (0, import_bar.bar)());

```