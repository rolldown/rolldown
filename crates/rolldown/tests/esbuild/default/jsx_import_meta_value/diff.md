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

```
### diff
```diff
===================================================================
--- esbuild	/out/factory.js
+++ rolldown	
@@ -1,5 +0,0 @@
-var import_meta = {};
-console.log([import_meta("x", null), import_meta("x", null)]);
-f = function () {
-    console.log([import_meta("y", null), import_meta("y", null)]);
-};

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

```
### diff
```diff
===================================================================
--- esbuild	/out/fragment.js
+++ rolldown	
@@ -1,4 +0,0 @@
-var import_meta = {};
-(console.log([import_meta(import_meta, null, "x"), import_meta(import_meta, null, "x")]), f = function () {
-    console.log([import_meta(import_meta, null, "y"), import_meta(import_meta, null, "y")]);
-});

```