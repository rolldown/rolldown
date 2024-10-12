# Diff
## /out.js
### esbuild
```js
// is-main.js
var require_is_main = __commonJS({
  "is-main.js"(exports2, module2) {
    module2.exports = require.main === module2;
  }
});

// entry.js
console.log("is main:", require.main === module);
console.log(require_is_main());
console.log("cache:", require.cache);
```
### rolldown
```js


//#region is-main.js
var require_is_main = __commonJS({ "is-main.js"(exports, module) {
	module.exports = require.main === module;
} });

//#endregion
//#region entry.js
var require_entry = __commonJS({ "entry.js"(exports, module) {
	console.log("is main:", require.main === module);
	console.log(require_is_main());
	console.log("cache:", require.cache);
} });

//#endregion
export default require_entry();


```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,8 +1,13 @@
 var require_is_main = __commonJS({
-    "is-main.js"(exports2, module2) {
-        module2.exports = require.main === module2;
+    "is-main.js"(exports, module) {
+        module.exports = require.main === module;
     }
 });
-console.log("is main:", require.main === module);
-console.log(require_is_main());
-console.log("cache:", require.cache);
+var require_entry = __commonJS({
+    "entry.js"(exports, module) {
+        console.log("is main:", require.main === module);
+        console.log(require_is_main());
+        console.log("cache:", require.cache);
+    }
+});
+export default require_entry();

```