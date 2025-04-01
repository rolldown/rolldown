# Diff
## /out.js
### esbuild
```js
// custom-react.js
function jsx() {
}
function Fragment() {
}

// entry.jsx
import { Fragment as Fragment2, jsx as jsx2 } from "react/jsx-runtime";
console.log(/* @__PURE__ */ jsx2("div", { jsx }), /* @__PURE__ */ jsx2(Fragment2, { children: /* @__PURE__ */ jsx2(Fragment, {}) }));
```
### rolldown
```js
import { Fragment, jsx } from "react/jsx-runtime";

//#region custom-react.js
function jsx$1() {}
function Fragment$1() {}
//#endregion

//#region entry.jsx
console.log(/* @__PURE__ */ jsx("div", { jsx: jsx$1 }), /* @__PURE__ */ jsx(Fragment, { children: /* @__PURE__ */ jsx(Fragment$1, {}) }));
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,8 +1,8 @@
-function jsx() {}
-function Fragment() {}
-import {Fragment as Fragment2, jsx as jsx2} from "react/jsx-runtime";
-console.log(jsx2("div", {
-    jsx
-}), jsx2(Fragment2, {
-    children: jsx2(Fragment, {})
+import {Fragment, jsx} from "react/jsx-runtime";
+function jsx$1() {}
+function Fragment$1() {}
+console.log(jsx("div", {
+    jsx: jsx$1
+}), jsx(Fragment, {
+    children: jsx(Fragment$1, {})
 }));

```