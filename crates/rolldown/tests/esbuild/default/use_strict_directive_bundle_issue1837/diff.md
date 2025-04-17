# Reason
1. inject path
# Diff
## /out.js
### esbuild
```js
(() => {
  // shims.js
  var import_process;
  var init_shims = __esm({
    "shims.js"() {
      import_process = __toESM(__require("process"));
    }
  });

  // cjs.js
  var require_cjs = __commonJS({
    "cjs.js"(exports) {
      "use strict";
      init_shims();
      exports.foo = import_process.default;
    }
  });

  // entry.js
  init_shims();
  console.log(require_cjs());
})();
```
### rolldown
```js
(function(node_assert) {

"use strict";
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

node_assert = __toESM(node_assert);

//#region cjs.js
var require_cjs = __commonJS({ "cjs.js"(exports) {
	exports.foo = process;
} });

//#region entry.js
node_assert.default.deepEqual(require_cjs(), { foo: process });

})(node_assert);
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,17 +1,36 @@
-(() => {
-    var import_process;
-    var init_shims = __esm({
-        "shims.js"() {
-            import_process = __toESM(__require("process"));
+(function (node_assert) {
+    var __create = Object.create;
+    var __defProp = Object.defineProperty;
+    var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
+    var __getOwnPropNames = Object.getOwnPropertyNames;
+    var __getProtoOf = Object.getPrototypeOf;
+    var __hasOwnProp = Object.prototype.hasOwnProperty;
+    var __commonJS = (cb, mod) => function () {
+        return (mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = {
+            exports: {}
+        }).exports, mod), mod.exports);
+    };
+    var __copyProps = (to, from, except, desc) => {
+        if (from && typeof from === "object" || typeof from === "function") for (var keys = __getOwnPropNames(from), i = 0, n = keys.length, key; i < n; i++) {
+            key = keys[i];
+            if (!__hasOwnProp.call(to, key) && key !== except) __defProp(to, key, {
+                get: (k => from[k]).bind(null, key),
+                enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable
+            });
         }
-    });
+        return to;
+    };
+    var __toESM = (mod, isNodeMode, target) => (target = mod != null ? __create(__getProtoOf(mod)) : {}, __copyProps(isNodeMode || !mod || !mod.__esModule ? __defProp(target, "default", {
+        value: mod,
+        enumerable: true
+    }) : target, mod));
+    node_assert = __toESM(node_assert);
     var require_cjs = __commonJS({
         "cjs.js"(exports) {
-            "use strict";
-            init_shims();
-            exports.foo = import_process.default;
+            exports.foo = process;
         }
     });
-    init_shims();
-    console.log(require_cjs());
-})();
+    node_assert.default.deepEqual(require_cjs(), {
+        foo: process
+    });
+})(node_assert);

```