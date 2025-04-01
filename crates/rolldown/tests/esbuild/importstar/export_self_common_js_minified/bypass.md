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



//#region entry.js
var require_entry = __commonJS({ "entry.js"(exports, module) {
	module.exports = { foo: 123 };
	console.log(require_entry());
} });
//#endregion

module.exports = require_entry();

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,7 +1,9 @@
-var r = s((f, e) => {
-    e.exports = {
-        foo: 123
-    };
-    console.log(r());
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