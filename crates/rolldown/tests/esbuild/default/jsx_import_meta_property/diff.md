# Diff
## /out/factory.js
### esbuild
```js
// factory.jsx
var import_meta = {};
console.log([
  /* @__PURE__ */ import_meta.factory("x", null),
  /* @__PURE__ */ import_meta.factory("x", null)
]);
f = function() {
  console.log([
    /* @__PURE__ */ import_meta.factory("y", null),
    /* @__PURE__ */ import_meta.factory("y", null)
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
-console.log([import_meta.factory("x", null), import_meta.factory("x", null)]);
-f = function () {
-    console.log([import_meta.factory("y", null), import_meta.factory("y", null)]);
-};

```
## /out/fragment.js
### esbuild
```js
// fragment.jsx
var import_meta = {};
console.log([
  /* @__PURE__ */ import_meta.factory(import_meta.fragment, null, "x"),
  /* @__PURE__ */ import_meta.factory(import_meta.fragment, null, "x")
]), f = function() {
  console.log([
    /* @__PURE__ */ import_meta.factory(import_meta.fragment, null, "y"),
    /* @__PURE__ */ import_meta.factory(import_meta.fragment, null, "y")
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
-(console.log([import_meta.factory(import_meta.fragment, null, "x"), import_meta.factory(import_meta.fragment, null, "x")]), f = function () {
-    console.log([import_meta.factory(import_meta.fragment, null, "y"), import_meta.factory(import_meta.fragment, null, "y")]);
-});

```