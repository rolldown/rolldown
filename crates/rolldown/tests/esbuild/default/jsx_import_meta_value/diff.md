# Reason
1. `jsx.factory`
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
import { jsx as _jsx } from "react/jsx-runtime";

//#region factory.jsx
console.log([_jsx("x", {}), /* @__PURE__ */ import.meta("x", null)]);
f = function() {
	console.log([_jsx("y", {}), /* @__PURE__ */ import.meta("y", null)]);
};

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/factory.js
+++ rolldown	factory.js
@@ -1,5 +1,5 @@
-var import_meta = {};
-console.log([import_meta("x", null), import_meta("x", null)]);
+import {jsx as _jsx} from "react/jsx-runtime";
+console.log([_jsx("x", {}), import.meta("x", null)]);
 f = function () {
-    console.log([import_meta("y", null), import_meta("y", null)]);
+    console.log([_jsx("y", {}), import.meta("y", null)]);
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
import { Fragment as _Fragment, jsx as _jsx } from "react/jsx-runtime";

//#region fragment.jsx
console.log([_jsx(_Fragment, { children: "x" }), /* @__PURE__ */ import.meta(import.meta, null, "x")]), f = function() {
	console.log([_jsx(_Fragment, { children: "y" }), /* @__PURE__ */ import.meta(import.meta, null, "y")]);
};

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/fragment.js
+++ rolldown	fragment.js
@@ -1,4 +1,8 @@
-var import_meta = {};
-(console.log([import_meta(import_meta, null, "x"), import_meta(import_meta, null, "x")]), f = function () {
-    console.log([import_meta(import_meta, null, "y"), import_meta(import_meta, null, "y")]);
+import {Fragment as _Fragment, jsx as _jsx} from "react/jsx-runtime";
+(console.log([_jsx(_Fragment, {
+    children: "x"
+}), import.meta(import.meta, null, "x")]), f = function () {
+    console.log([_jsx(_Fragment, {
+        children: "y"
+    }), import.meta(import.meta, null, "y")]);
 });

```