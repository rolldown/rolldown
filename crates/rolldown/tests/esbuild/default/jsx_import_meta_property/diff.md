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
import { jsx as _jsx } from "react/jsx-runtime";

//#region factory.jsx
console.log([_jsx("x", {}), /* @__PURE__ */ import.meta.factory("x", null)]);
f = function() {
	console.log([_jsx("y", {}), /* @__PURE__ */ import.meta.factory("y", null)]);
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
-console.log([import_meta.factory("x", null), import_meta.factory("x", null)]);
+import {jsx as _jsx} from "react/jsx-runtime";
+console.log([_jsx("x", {}), import.meta.factory("x", null)]);
 f = function () {
-    console.log([import_meta.factory("y", null), import_meta.factory("y", null)]);
+    console.log([_jsx("y", {}), import.meta.factory("y", null)]);
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
import { Fragment as _Fragment, jsx as _jsx } from "react/jsx-runtime";

//#region fragment.jsx
console.log([_jsx(_Fragment, { children: "x" }), /* @__PURE__ */ import.meta.factory(import.meta.fragment, null, "x")]), f = function() {
	console.log([_jsx(_Fragment, { children: "y" }), /* @__PURE__ */ import.meta.factory(import.meta.fragment, null, "y")]);
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
-(console.log([import_meta.factory(import_meta.fragment, null, "x"), import_meta.factory(import_meta.fragment, null, "x")]), f = function () {
-    console.log([import_meta.factory(import_meta.fragment, null, "y"), import_meta.factory(import_meta.fragment, null, "y")]);
+import {Fragment as _Fragment, jsx as _jsx} from "react/jsx-runtime";
+(console.log([_jsx(_Fragment, {
+    children: "x"
+}), import.meta.factory(import.meta.fragment, null, "x")]), f = function () {
+    console.log([_jsx(_Fragment, {
+        children: "y"
+    }), import.meta.factory(import.meta.fragment, null, "y")]);
 });

```