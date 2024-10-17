# Reason
1. `jsx.factory`
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

//#region factory.jsx
console.log([React.createElement("x", null), /* @__PURE__ */ import.meta.factory("x", null)]);
f = function() {
	console.log([React.createElement("y", null), /* @__PURE__ */ import.meta.factory("y", null)]);
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
-console.log([import_meta.factory("x", null), import_meta.factory("x", null)]);
+console.log([React.createElement("x", null), import.meta.factory("x", null)]);
 f = function () {
-    console.log([import_meta.factory("y", null), import_meta.factory("y", null)]);
+    console.log([React.createElement("y", null), import.meta.factory("y", null)]);
 };

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

//#region fragment.jsx
console.log([React.createElement(React.Fragment, null, "x"), /* @__PURE__ */ import.meta.factory(import.meta.fragment, null, "x")]), f = function() {
	console.log([React.createElement(React.Fragment, null, "y"), /* @__PURE__ */ import.meta.factory(import.meta.fragment, null, "y")]);
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
-(console.log([import_meta.factory(import_meta.fragment, null, "x"), import_meta.factory(import_meta.fragment, null, "x")]), f = function () {
-    console.log([import_meta.factory(import_meta.fragment, null, "y"), import_meta.factory(import_meta.fragment, null, "y")]);
+(console.log([React.createElement(React.Fragment, null, "x"), import.meta.factory(import.meta.fragment, null, "x")]), f = function () {
+    console.log([React.createElement(React.Fragment, null, "y"), import.meta.factory(import.meta.fragment, null, "y")]);
 });

```