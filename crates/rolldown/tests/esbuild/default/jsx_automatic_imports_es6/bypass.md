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
import { Fragment as Fragment$1, jsx as jsx$1 } from "react/jsx-runtime";

//#region custom-react.js
function jsx$2() {}
function Fragment$2() {}

//#endregion
//#region entry.jsx
console.log(/* @__PURE__ */ jsx$1("div", { jsx: jsx$2 }), /* @__PURE__ */ jsx$1(Fragment$1, { children: /* @__PURE__ */ jsx$1(Fragment$2, {}) }));

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
+import {Fragment as Fragment$1, jsx as jsx$1} from "react/jsx-runtime";
+function jsx$2() {}
+function Fragment$2() {}
+console.log(jsx$1("div", {
+    jsx: jsx$2
+}), jsx$1(Fragment$1, {
+    children: jsx$1(Fragment$2, {})
 }));

```