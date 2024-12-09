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
	jsx("div", { x }),
	jsx("div", { x: "y" }),
	jsx("div", { x: true }),
	jsx("div", { ...x }),
	jsx("div", { children: x }),
	jsx(Fragment, { children: x }),
	// Comments on absent AST nodes
	jsxs("div", { children: ["before", "after"] }),
	jsxs("div", { children: ["before", "after"] }),
	jsxs("div", { children: ["before", "after"] }),
	jsxs(Fragment, { children: ["before", "after"] }),
	jsxs(Fragment, { children: ["before", "after"] }),
	jsxs(Fragment, { children: ["before", "after"] })
);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,46 +1,20 @@
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
+	jsx("div", { x }),
+	jsx("div", { x: "y" }),
+	jsx("div", { x: true }),
+	jsx("div", { ...x }),
+	jsx("div", { children: x }),
+	jsx(Fragment, { children: x }),
+	// Comments on absent AST nodes
+	jsxs("div", { children: ["before", "after"] }),
+	jsxs("div", { children: ["before", "after"] }),
+	jsxs("div", { children: ["before", "after"] }),
+	jsxs(Fragment, { children: ["before", "after"] }),
+	jsxs(Fragment, { children: ["before", "after"] }),
+	jsxs(Fragment, { children: ["before", "after"] })
+);
+
+//#endregion
\ No newline at end of file

```