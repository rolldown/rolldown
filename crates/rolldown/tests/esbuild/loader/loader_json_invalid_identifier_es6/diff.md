# Reason
1. json partial namespace memberExpr used tree shaking
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


//#region test.json
var test_exports = {};
__export(test_exports, {
	default: () => test_default,
	"invalid-identifier": () => invalid_identifier$1
});
var invalid_identifier$1 = true;
var test_default = { "invalid-identifier": invalid_identifier$1 };

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
console.log(test_exports["invalid-identifier"], test2_exports);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,11 +1,19 @@
-var invalid_identifier = true;
+var test_exports = {};
+__export(test_exports, {
+    default: () => test_default,
+    "invalid-identifier": () => invalid_identifier$1
+});
+var invalid_identifier$1 = true;
+var test_default = {
+    "invalid-identifier": invalid_identifier$1
+};
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
+console.log(test_exports["invalid-identifier"], test2_exports);

```