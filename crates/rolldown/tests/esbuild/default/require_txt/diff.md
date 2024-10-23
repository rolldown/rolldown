# Reason
1. `.txt` module should be treated as cjs
# Diff
## /out.js
### esbuild
```js
// test.txt
var require_test = __commonJS({
  "test.txt"(exports, module) {
    module.exports = "This is a test.";
  }
});

// entry.js
console.log(require_test());
```
### rolldown
```js


//#region test.txt
var require_test = __commonJS({ "test.txt"() {
	module.exports = "This is a test.";
} });

//#endregion
//#region entry.js
console.log(require_test());

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,6 +1,6 @@
 var require_test = __commonJS({
-    "test.txt"(exports, module) {
+    "test.txt"() {
         module.exports = "This is a test.";
     }
 });
 console.log(require_test());

```