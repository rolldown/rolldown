# Reason
1. not support `jsx.preserve`
# Diff
## /out/entry.js
### esbuild
```js
// entry.jsx
console.log(
  <div x={
    /*before*/
    x
  } />,
  <div x={
    /*before*/
    "y"
  } />,
  <div x={
    /*before*/
    true
  } />,
  <div {
    /*before*/
    ...x
  } />,
  <div>{
    /*before*/
    x
  }</div>,
  <>{
    /*before*/
    x
  }</>,
  // Comments on absent AST nodes
  <div>before{}after</div>,
  <div>before{
    /* comment 1 */
    /* comment 2 */
  }after</div>,
  <div>before{
    // comment 1
    // comment 2
  }after</div>,
  <>before{}after</>,
  <>before{
    /* comment 1 */
    /* comment 2 */
  }after</>,
  <>before{
    // comment 1
    // comment 2
  }after</>
);
```
### rolldown
```js
import { Fragment, jsx, jsxs } from "react/jsx-runtime";

//#region entry.jsx
console.log(
	/* @__PURE__ */ jsx("div", { x }),
	/* @__PURE__ */ jsx("div", { x: "y" }),
	/* @__PURE__ */ jsx("div", { x: true }),
	/* @__PURE__ */ jsx("div", { ...x }),
	/* @__PURE__ */ jsx("div", { children: x }),
	/* @__PURE__ */ jsx(Fragment, { children: x }),
	// Comments on absent AST nodes
	/* @__PURE__ */ jsxs("div", { children: ["before", "after"] }),
	/* @__PURE__ */ jsxs("div", { children: ["before", "after"] }),
	/* @__PURE__ */ jsxs("div", { children: ["before", "after"] }),
	/* @__PURE__ */ jsxs(Fragment, { children: ["before", "after"] }),
	/* @__PURE__ */ jsxs(Fragment, { children: ["before", "after"] }),
	/* @__PURE__ */ jsxs(Fragment, { children: ["before", "after"] })
);

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,46 +1,18 @@
-// entry.jsx
+import { Fragment, jsx, jsxs } from "react/jsx-runtime";
+
+//#region entry.jsx
 console.log(
-  <div x={
-    /*before*/
-    x
-  } />,
-  <div x={
-    /*before*/
-    "y"
-  } />,
-  <div x={
-    /*before*/
-    true
-  } />,
-  <div {
-    /*before*/
-    ...x
-  } />,
-  <div>{
-    /*before*/
-    x
-  }</div>,
-  <>{
-    /*before*/
-    x
-  }</>,
-  // Comments on absent AST nodes
-  <div>before{}after</div>,
-  <div>before{
-    /* comment 1 */
-    /* comment 2 */
-  }after</div>,
-  <div>before{
-    // comment 1
-    // comment 2
-  }after</div>,
-  <>before{}after</>,
-  <>before{
-    /* comment 1 */
-    /* comment 2 */
-  }after</>,
-  <>before{
-    // comment 1
-    // comment 2
-  }after</>
-);
\ No newline at end of file
+	/* @__PURE__ */ jsx("div", { x }),
+	/* @__PURE__ */ jsx("div", { x: "y" }),
+	/* @__PURE__ */ jsx("div", { x: true }),
+	/* @__PURE__ */ jsx("div", { ...x }),
+	/* @__PURE__ */ jsx("div", { children: x }),
+	/* @__PURE__ */ jsx(Fragment, { children: x }),
+	// Comments on absent AST nodes
+	/* @__PURE__ */ jsxs("div", { children: ["before", "after"] }),
+	/* @__PURE__ */ jsxs("div", { children: ["before", "after"] }),
+	/* @__PURE__ */ jsxs("div", { children: ["before", "after"] }),
+	/* @__PURE__ */ jsxs(Fragment, { children: ["before", "after"] }),
+	/* @__PURE__ */ jsxs(Fragment, { children: ["before", "after"] }),
+	/* @__PURE__ */ jsxs(Fragment, { children: ["before", "after"] })
+);

```