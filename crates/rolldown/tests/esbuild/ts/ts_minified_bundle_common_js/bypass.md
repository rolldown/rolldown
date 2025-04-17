# Reason
1. could be done in minifier
# Diff
## /out.js
### esbuild
```js
var t=e(r=>{r.foo=function(){return 123}});var n=e((l,c)=>{c.exports={test:!0}});var{foo:f}=t();console.log(f(),n());
```
### rolldown
```js

//#region rolldown:runtime
var __getOwnPropNames = Object.getOwnPropertyNames;
var __commonJS = (cb, mod) => function() {
	return mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod), mod.exports;
};

//#region a.ts
var require_a = __commonJS({ "a.ts"(exports) {
	exports.foo = function() {
		return 123;
	};
} });

//#region j.json
var require_j = __commonJS({ "j.json"(exports, module) {
	module.exports = { "test": true };
} });

//#region entry.ts
const { foo } = require_a();
console.log(foo(), require_j());

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,12 +1,22 @@
-var t = e(r => {
-    r.foo = function () {
-        return 123;
-    };
+var __getOwnPropNames = Object.getOwnPropertyNames;
+var __commonJS = (cb, mod) => function () {
+    return (mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = {
+        exports: {}
+    }).exports, mod), mod.exports);
+};
+var require_a = __commonJS({
+    "a.ts"(exports) {
+        exports.foo = function () {
+            return 123;
+        };
+    }
 });
-var n = e((l, c) => {
-    c.exports = {
-        test: !0
-    };
+var require_j = __commonJS({
+    "j.json"(exports, module) {
+        module.exports = {
+            "test": true
+        };
+    }
 });
-var {foo: f} = t();
-console.log(f(), n());
+var {foo} = require_a();
+console.log(foo(), require_j());

```