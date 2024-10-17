# Reason
1. comments codegen
# Diff
## /out/entry.js
### esbuild
```js
// entry.jsx
console.log(
  /* @__PURE__ */ React.createElement("div", { x: (
    /*before*/
    x
  ) }),
  /* @__PURE__ */ React.createElement("div", { x: (
    /*before*/
    "y"
  ) }),
  /* @__PURE__ */ React.createElement("div", { x: (
    /*before*/
    true
  ) }),
  /* @__PURE__ */ React.createElement("div", {
    /*before*/
    ...x
  }),
  /* @__PURE__ */ React.createElement(
    "div",
    null,
    /*before*/
    x
  ),
  /* @__PURE__ */ React.createElement(
    React.Fragment,
    null,
    /*before*/
    x
  ),
  // Comments on absent AST nodes
  /* @__PURE__ */ React.createElement("div", null, "before", "after"),
  /* @__PURE__ */ React.createElement("div", null, "before", "after"),
  /* @__PURE__ */ React.createElement("div", null, "before", "after"),
  /* @__PURE__ */ React.createElement(React.Fragment, null, "before", "after"),
  /* @__PURE__ */ React.createElement(React.Fragment, null, "before", "after"),
  /* @__PURE__ */ React.createElement(React.Fragment, null, "before", "after")
);
```
### rolldown
```js
import { Fragment as _Fragment, jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";

//#region entry.jsx
console.log(
	_jsx("div", { x }),
	_jsx("div", { x: "y" }),
	_jsx("div", { x: true }),
	_jsx("div", { ...x }),
	_jsx("div", { children: x }),
	_jsx(_Fragment, { children: x }),
	// Comments on absent AST nodes
	_jsxs("div", { children: ["before", "after"] }),
	_jsxs("div", { children: ["before", "after"] }),
	_jsxs("div", { children: ["before", "after"] }),
	_jsxs(_Fragment, { children: ["before", "after"] }),
	_jsxs(_Fragment, { children: ["before", "after"] }),
	_jsxs(_Fragment, { children: ["before", "after"] })
);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,9 +1,26 @@
-console.log(React.createElement("div", {
-    x: x
-}), React.createElement("div", {
+import {Fragment as _Fragment, jsx as _jsx, jsxs as _jsxs} from "react/jsx-runtime";
+console.log(_jsx("div", {
+    x
+}), _jsx("div", {
     x: "y"
-}), React.createElement("div", {
+}), _jsx("div", {
     x: true
-}), React.createElement("div", {
+}), _jsx("div", {
     ...x
-}), React.createElement("div", null, x), React.createElement(React.Fragment, null, x), React.createElement("div", null, "before", "after"), React.createElement("div", null, "before", "after"), React.createElement("div", null, "before", "after"), React.createElement(React.Fragment, null, "before", "after"), React.createElement(React.Fragment, null, "before", "after"), React.createElement(React.Fragment, null, "before", "after"));
+}), _jsx("div", {
+    children: x
+}), _jsx(_Fragment, {
+    children: x
+}), _jsxs("div", {
+    children: ["before", "after"]
+}), _jsxs("div", {
+    children: ["before", "after"]
+}), _jsxs("div", {
+    children: ["before", "after"]
+}), _jsxs(_Fragment, {
+    children: ["before", "after"]
+}), _jsxs(_Fragment, {
+    children: ["before", "after"]
+}), _jsxs(_Fragment, {
+    children: ["before", "after"]
+}));

```