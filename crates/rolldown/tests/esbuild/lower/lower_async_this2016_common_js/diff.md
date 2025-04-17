# Diff
## /out.js
### esbuild
```js
// entry.js
var require_entry = __commonJS({
  "entry.js"(exports) {
    exports.foo = () => __async(exports, null, function* () {
      return exports;
    });
  }
});
export default require_entry();
```
### rolldown
```js

//#region rolldown:runtime
var __getOwnPropNames = Object.getOwnPropertyNames;
var __commonJS = (cb, mod) => function() {
	return mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod), mod.exports;
};

//#region entry.js
var require_entry = __commonJS({ "entry.js"(exports) {
	exports.foo = async () => exports;
} });

export default require_entry();

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,8 +1,12 @@
+var __getOwnPropNames = Object.getOwnPropertyNames;
+var __commonJS = (cb, mod) => function () {
+    return (mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = {
+        exports: {}
+    }).exports, mod), mod.exports);
+};
 var require_entry = __commonJS({
     "entry.js"(exports) {
-        exports.foo = () => __async(exports, null, function* () {
-            return exports;
-        });
+        exports.foo = async () => exports;
     }
 });
 export default require_entry();

```