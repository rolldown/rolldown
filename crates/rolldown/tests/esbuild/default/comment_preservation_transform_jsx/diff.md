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

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	
@@ -1,9 +0,0 @@
-console.log(React.createElement("div", {
-    x: x
-}), React.createElement("div", {
-    x: "y"
-}), React.createElement("div", {
-    x: true
-}), React.createElement("div", {
-    ...x
-}), React.createElement("div", null, x), React.createElement(React.Fragment, null, x), React.createElement("div", null, "before", "after"), React.createElement("div", null, "before", "after"), React.createElement("div", null, "before", "after"), React.createElement(React.Fragment, null, "before", "after"), React.createElement(React.Fragment, null, "before", "after"), React.createElement(React.Fragment, null, "before", "after"));

```