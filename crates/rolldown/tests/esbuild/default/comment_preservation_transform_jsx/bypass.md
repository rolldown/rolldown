# Reason
1. transpiled jsx should have leading `@__PURE__`, already tracked https://github.com/oxc-project/oxc/issues/6072
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

//#region entry.jsx
console.log(
	/* @__PURE__ */ React.createElement("div", { x }),
	/* @__PURE__ */ React.createElement("div", { x: "y" }),
	/* @__PURE__ */ React.createElement("div", { x: true }),
	/* @__PURE__ */ React.createElement("div", x),
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
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,9 +1,7 @@
 console.log(React.createElement("div", {
-    x: x
+    x
 }), React.createElement("div", {
     x: "y"
 }), React.createElement("div", {
     x: true
-}), React.createElement("div", {
-    ...x
-}), React.createElement("div", null, x), React.createElement(React.Fragment, null, x), React.createElement("div", null, "before", "after"), React.createElement("div", null, "before", "after"), React.createElement("div", null, "before", "after"), React.createElement(React.Fragment, null, "before", "after"), React.createElement(React.Fragment, null, "before", "after"), React.createElement(React.Fragment, null, "before", "after"));
+}), React.createElement("div", x), React.createElement("div", null, x), React.createElement(React.Fragment, null, x), React.createElement("div", null, "before", "after"), React.createElement("div", null, "before", "after"), React.createElement("div", null, "before", "after"), React.createElement(React.Fragment, null, "before", "after"), React.createElement(React.Fragment, null, "before", "after"), React.createElement(React.Fragment, null, "before", "after"));

```