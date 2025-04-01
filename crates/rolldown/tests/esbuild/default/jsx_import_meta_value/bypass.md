# Reason
1. don't support `unsupportedFeature` https://github.com/evanw/esbuild/commit/71a2f8de5ad4e1882f35c449efa25761aa1241b5#diff-e20508c4ae566a2d8a60274ff05e408d81c9758a27d84318feecdfbf9e24af5eR11297-R11308
# Diff
## /out/factory.js
### esbuild
```js
// factory.jsx
var import_meta = {};
console.log([
  /* @__PURE__ */ import_meta("x", null),
  /* @__PURE__ */ import_meta("x", null)
]);
f = function() {
  console.log([
    /* @__PURE__ */ import_meta("y", null),
    /* @__PURE__ */ import_meta("y", null)
  ]);
};
```
### rolldown
```js

//#region factory.jsx
console.log([/* @__PURE__ */ import.meta("x", null), /* @__PURE__ */ import.meta("x", null)]);
f = function() {
	console.log([/* @__PURE__ */ import.meta("y", null), /* @__PURE__ */ import.meta("y", null)]);
};
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/factory.js
+++ rolldown	factory.js
@@ -1,5 +1,4 @@
-var import_meta = {};
-console.log([import_meta("x", null), import_meta("x", null)]);
+console.log([import.meta("x", null), import.meta("x", null)]);
 f = function () {
-    console.log([import_meta("y", null), import_meta("y", null)]);
+    console.log([import.meta("y", null), import.meta("y", null)]);
 };

```
## /out/fragment.js
### esbuild
```js
// fragment.jsx
var import_meta = {};
console.log([
  /* @__PURE__ */ import_meta(import_meta, null, "x"),
  /* @__PURE__ */ import_meta(import_meta, null, "x")
]), f = function() {
  console.log([
    /* @__PURE__ */ import_meta(import_meta, null, "y"),
    /* @__PURE__ */ import_meta(import_meta, null, "y")
  ]);
};
```
### rolldown
```js

//#region fragment.jsx
console.log([/* @__PURE__ */ import.meta(import.meta, null, "x"), /* @__PURE__ */ import.meta(import.meta, null, "x")]), f = function() {
	console.log([/* @__PURE__ */ import.meta(import.meta, null, "y"), /* @__PURE__ */ import.meta(import.meta, null, "y")]);
};
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/fragment.js
+++ rolldown	fragment.js
@@ -1,4 +1,3 @@
-var import_meta = {};
-(console.log([import_meta(import_meta, null, "x"), import_meta(import_meta, null, "x")]), f = function () {
-    console.log([import_meta(import_meta, null, "y"), import_meta(import_meta, null, "y")]);
+(console.log([import.meta(import.meta, null, "x"), import.meta(import.meta, null, "x")]), f = function () {
+    console.log([import.meta(import.meta, null, "y"), import.meta(import.meta, null, "y")]);
 });

```