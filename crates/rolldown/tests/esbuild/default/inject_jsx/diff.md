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
import { Fragment as _Fragment, jsx as _jsx } from "react/jsx-runtime";

//#region entry.jsx
console.log(_jsx(_Fragment, { children: _jsx("div", {}) }));

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,3 +1,4 @@
-function el() {}
-function frag() {}
-console.log(el(frag, null, el("div", null)));
+import {Fragment as _Fragment, jsx as _jsx} from "react/jsx-runtime";
+console.log(_jsx(_Fragment, {
+    children: _jsx("div", {})
+}));

```