# Reason
1. `jsx.factory`
# Diff
## /out/factory.js
### esbuild
```js
// factory.jsx
console.log([
  /* @__PURE__ */ (void 0)("x", null),
  /* @__PURE__ */ (void 0)("x", null)
]);
f = function() {
  console.log([
    /* @__PURE__ */ this("y", null),
    /* @__PURE__ */ this("y", null)
  ]);
};
```
### rolldown
```js
import { jsx as _jsx } from "react/jsx-runtime";

//#region factory.jsx
console.log([_jsx("x", {}), /* @__PURE__ */ this("x", null)]);
f = function() {
	console.log([_jsx("y", {}), /* @__PURE__ */ this("y", null)]);
};

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/factory.js
+++ rolldown	factory.js
@@ -1,4 +1,5 @@
-console.log([(void 0)("x", null), (void 0)("x", null)]);
+import {jsx as _jsx} from "react/jsx-runtime";
+console.log([_jsx("x", {}), this("x", null)]);
 f = function () {
-    console.log([this("y", null), this("y", null)]);
+    console.log([_jsx("y", {}), this("y", null)]);
 };

```
## /out/fragment.js
### esbuild
```js
// fragment.jsx
console.log([
  /* @__PURE__ */ (void 0)(void 0, null, "x"),
  /* @__PURE__ */ (void 0)(void 0, null, "x")
]), f = function() {
  console.log([
    /* @__PURE__ */ this(this, null, "y"),
    /* @__PURE__ */ this(this, null, "y")
  ]);
};
```
### rolldown
```js
import { Fragment as _Fragment, jsx as _jsx } from "react/jsx-runtime";

//#region fragment.jsx
console.log([_jsx(_Fragment, { children: "x" }), /* @__PURE__ */ this(this, null, "x")]), f = function() {
	console.log([_jsx(_Fragment, { children: "y" }), /* @__PURE__ */ this(this, null, "y")]);
};

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/fragment.js
+++ rolldown	fragment.js
@@ -1,3 +1,8 @@
-(console.log([(void 0)(void 0, null, "x"), (void 0)(void 0, null, "x")]), f = function () {
-    console.log([this(this, null, "y"), this(this, null, "y")]);
+import {Fragment as _Fragment, jsx as _jsx} from "react/jsx-runtime";
+(console.log([_jsx(_Fragment, {
+    children: "x"
+}), this(this, null, "x")]), f = function () {
+    console.log([_jsx(_Fragment, {
+        children: "y"
+    }), this(this, null, "y")]);
 });

```