# Reason
1. could be done in minifier
# Diff
## /out/import.js
### esbuild
```js
var o=123;console.log(o,"no identifier in this file should be named W, X, Y, or Z");
```
### rolldown
```js

//#region WWWWWWWWWWXXXXXXXXXXYYYYYYYYYYZZZZZZZZZZ.js
var WWWWWWWWWWXXXXXXXXXXYYYYYYYYYYZZZZZZZZZZ_default = 123;
//#endregion

//#region import.js
console.log(WWWWWWWWWWXXXXXXXXXXYYYYYYYYYYZZZZZZZZZZ_default, "no identifier in this file should be named W, X, Y, or Z");
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/import.js
+++ rolldown	import.js
@@ -1,2 +1,2 @@
-var o = 123;
-console.log(o, "no identifier in this file should be named W, X, Y, or Z");
+var WWWWWWWWWWXXXXXXXXXXYYYYYYYYYYZZZZZZZZZZ_default = 123;
+console.log(WWWWWWWWWWXXXXXXXXXXYYYYYYYYYYZZZZZZZZZZ_default, "no identifier in this file should be named W, X, Y, or Z");

```
## /out/require.js
### esbuild
```js
var i=r((t,e)=>{e.exports=123});var s=i();console.log(s,"no identifier in this file should be named A, B, C, or D");
```
### rolldown
```js



//#region AAAAAAAAAABBBBBBBBBBCCCCCCCCCCDDDDDDDDDD.js
var require_AAAAAAAAAABBBBBBBBBBCCCCCCCCCCDDDDDDDDDD = __commonJS({ "AAAAAAAAAABBBBBBBBBBCCCCCCCCCCDDDDDDDDDD.js"(exports, module) {
	module.exports = 123;
} });
//#endregion

//#region require.js
const foo = require_AAAAAAAAAABBBBBBBBBBCCCCCCCCCCDDDDDDDDDD();
console.log(foo, "no identifier in this file should be named A, B, C, or D");
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/require.js
+++ rolldown	require.js
@@ -1,5 +1,7 @@
-var i = r((t, e) => {
-    e.exports = 123;
+var require_AAAAAAAAAABBBBBBBBBBCCCCCCCCCCDDDDDDDDDD = __commonJS({
+    "AAAAAAAAAABBBBBBBBBBCCCCCCCCCCDDDDDDDDDD.js"(exports, module) {
+        module.exports = 123;
+    }
 });
-var s = i();
-console.log(s, "no identifier in this file should be named A, B, C, or D");
+var foo = require_AAAAAAAAAABBBBBBBBBBCCCCCCCCCCDDDDDDDDDD();
+console.log(foo, "no identifier in this file should be named A, B, C, or D");

```