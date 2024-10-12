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
import { Fragment as _Fragment, jsx as _jsx } from "react/jsx-runtime";

//#region custom-react.js
function jsx() {}
function Fragment() {}

//#endregion
//#region entry.jsx
console.log(_jsx("div", { jsx }), _jsx(_Fragment, { children: _jsx(Fragment, {}) }));

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,8 +1,8 @@
+import {Fragment as _Fragment, jsx as _jsx} from "react/jsx-runtime";
 function jsx() {}
 function Fragment() {}
-import {Fragment as Fragment2, jsx as jsx2} from "react/jsx-runtime";
-console.log(jsx2("div", {
+console.log(_jsx("div", {
     jsx
-}), jsx2(Fragment2, {
-    children: jsx2(Fragment, {})
+}), _jsx(_Fragment, {
+    children: _jsx(Fragment, {})
 }));

```