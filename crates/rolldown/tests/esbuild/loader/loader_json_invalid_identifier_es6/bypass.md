# Reason
1. different codegen order
# Diff
## /out.js
### esbuild
```js
// test.json
var invalid_identifier = true;

// test2.json
var test2_exports = {};
__export(test2_exports, {
  default: () => test2_default,
  "invalid-identifier": () => invalid_identifier2
});
var invalid_identifier2 = true;
var test2_default = { "invalid-identifier": invalid_identifier2 };

// entry.js
console.log(invalid_identifier, test2_exports);
```
### rolldown
```js
// HIDDEN [rolldown:runtime]
//#region test.json
var invalid_identifier$1 = true;

//#endregion
//#region test2.json
var test2_exports = {};
__export(test2_exports, {
	default: () => test2_default,
	"invalid-identifier": () => invalid_identifier
});
var invalid_identifier = true;
var test2_default = { "invalid-identifier": invalid_identifier };

//#endregion
//#region entry.js
console.log(invalid_identifier$1, test2_exports);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,11 +1,11 @@
-var invalid_identifier = true;
+var invalid_identifier$1 = true;
 var test2_exports = {};
 __export(test2_exports, {
     default: () => test2_default,
-    "invalid-identifier": () => invalid_identifier2
+    "invalid-identifier": () => invalid_identifier
 });
-var invalid_identifier2 = true;
+var invalid_identifier = true;
 var test2_default = {
-    "invalid-identifier": invalid_identifier2
+    "invalid-identifier": invalid_identifier
 };
-console.log(invalid_identifier, test2_exports);
+console.log(invalid_identifier$1, test2_exports);

```