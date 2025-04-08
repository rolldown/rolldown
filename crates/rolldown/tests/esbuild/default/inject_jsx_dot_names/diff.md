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

//#region inject.js
function $inject_React_createElement() {}
function $inject_React_Fragment() {}

//#endregion
//#region entry.jsx
console.log(/* @__PURE__ */ $inject_React_createElement($inject_React_Fragment, null, /* @__PURE__ */ $inject_React_createElement("div", null)));

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,3 +1,3 @@
-function el() {}
-function frag() {}
-console.log(el(frag, null, el("div", null)));
+function $inject_React_createElement() {}
+function $inject_React_Fragment() {}
+console.log($inject_React_createElement($inject_React_Fragment, null, $inject_React_createElement("div", null)));

```