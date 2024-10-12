# Diff
## /out.js
### esbuild
```js
// import.ts
var value = 123;

// entry.ts
var value_copy = value;
var foo = value_copy;
console.log(foo);
```
### rolldown
```js


//#region import.ts
var import_exports = {};
__export(import_exports, { value: () => value });
let value = 123;

//#endregion
//#region entry.ts
var value_copy = import_exports.value;
var Type_copy = import_exports.Type;
let foo = value_copy;
console.log(foo);

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,4 +1,9 @@
+var import_exports = {};
+__export(import_exports, {
+    value: () => value
+});
 var value = 123;
-var value_copy = value;
+var value_copy = import_exports.value;
+var Type_copy = import_exports.Type;
 var foo = value_copy;
 console.log(foo);

```