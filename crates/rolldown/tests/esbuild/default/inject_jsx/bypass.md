# Reason
1. due to multi pass transformer arch, this test could not be supported for now(we should `Define` first and then `Transform`).
# Diff
## /out.js
### esbuild
```js
// inject.js
function el() {
}
function frag() {
}

// entry.jsx
console.log(/* @__PURE__ */ el(frag, null, /* @__PURE__ */ el("div", null)));
```
### rolldown
```js

//#region entry.jsx
console.log(/* @__PURE__ */ React.createElement(React.Fragment, null, /* @__PURE__ */ React.createElement("div", null)));
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,3 +1,1 @@
-function el() {}
-function frag() {}
-console.log(el(frag, null, el("div", null)));
+console.log(React.createElement(React.Fragment, null, React.createElement("div", null)));

```