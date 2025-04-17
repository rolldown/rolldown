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

//#region rolldown:runtime
var __defProp = Object.defineProperty;
var __export = (target, all) => {
	for (var name in all) __defProp(target, name, {
		get: all[name],
		enumerable: true
	});
};

//#region test.json
var invalid_identifier$1 = true;

//#region test2.json
var test2_exports = {};
__export(test2_exports, {
	default: () => test2_default,
	"invalid-identifier": () => invalid_identifier
});
var invalid_identifier = true;
var test2_default = { "invalid-identifier": invalid_identifier };

//#region entry.js
console.log(invalid_identifier$1, test2_exports);

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,11 +1,18 @@
-var invalid_identifier = true;
+var __defProp = Object.defineProperty;
+var __export = (target, all) => {
+    for (var name in all) __defProp(target, name, {
+        get: all[name],
+        enumerable: true
+    });
+};
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