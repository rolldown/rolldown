# Reason 
1. Rolldown doesn't minify symbols.
# Diff
## /out.js
### esbuild
```js
// entry.js
var r = s((f, e) => {
  e.exports = { foo: 123 };
  console.log(r());
});
module.exports = r();
```
### rolldown
```js
//#region rolldown:runtime
var __getOwnPropNames = Object.getOwnPropertyNames;
var __commonJS = (cb, mod) => function() {
	return mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod), mod.exports;
};


//#region entry.js
var require_entry = __commonJS({ "entry.js"(exports, module) {
	module.exports = { foo: 123 };
	console.log(require_entry());
} });

module.exports = require_entry();

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,7 +1,15 @@
-var r = s((f, e) => {
-    e.exports = {
-        foo: 123
-    };
-    console.log(r());
+var __getOwnPropNames = Object.getOwnPropertyNames;
+var __commonJS = (cb, mod) => function () {
+    return (mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = {
+        exports: {}
+    }).exports, mod), mod.exports);
+};
+var require_entry = __commonJS({
+    "entry.js"(exports, module) {
+        module.exports = {
+            foo: 123
+        };
+        console.log(require_entry());
+    }
 });
-module.exports = r();
+module.exports = require_entry();

```