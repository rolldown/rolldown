# Reason
1. `.custom` should be treated as cjs
# Diff
## /out.js
### esbuild
```js
// test.custom
var require_test = __commonJS({
  "test.custom"(exports, module) {
    module.exports = "#include <stdio.h>";
  }
});

// entry.js
console.log(require_test());
```
### rolldown
```js


//#region test.custom
var require_test = __commonJS({ "test.custom"() {
	module.exports = "#include <stdio.h>";
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
-    "test.custom"(exports, module) {
+    "test.custom"() {
         module.exports = "#include <stdio.h>";
     }
 });
 console.log(require_test());

```