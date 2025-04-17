# Reason
1. skip quote
# Diff
## /out.js
### esbuild
```js
// test.json
var require_test = __commonJS({
  "test.json"(exports, module) {
    module.exports = {
      a: true,
      b: 123,
      c: [null]
    };
  }
});

// entry.js
console.log(require_test());
```
### rolldown
```js

//#region rolldown:runtime
var __getOwnPropNames = Object.getOwnPropertyNames;
var __commonJS = (cb, mod) => function() {
	return mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod), mod.exports;
};

//#region test.json
var require_test = __commonJS({ "test.json"(exports, module) {
	module.exports = {
		"a": true,
		"b": 123,
		"c": [null]
	};
} });

//#region entry.js
console.log(require_test());

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,10 +1,16 @@
+var __getOwnPropNames = Object.getOwnPropertyNames;
+var __commonJS = (cb, mod) => function () {
+    return (mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = {
+        exports: {}
+    }).exports, mod), mod.exports);
+};
 var require_test = __commonJS({
     "test.json"(exports, module) {
         module.exports = {
-            a: true,
-            b: 123,
-            c: [null]
+            "a": true,
+            "b": 123,
+            "c": [null]
         };
     }
 });
 console.log(require_test());

```