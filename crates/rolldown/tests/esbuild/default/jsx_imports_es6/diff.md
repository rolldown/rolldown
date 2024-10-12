# Diff
## /out.js
### esbuild
```js
// custom-react.js
function elem() {
}
function frag() {
}

// entry.jsx
console.log(/* @__PURE__ */ elem("div", null), /* @__PURE__ */ elem(frag, null, "fragment"));
```
### rolldown
```js
import { Fragment as _Fragment, jsx as _jsx } from "react/jsx-runtime";

//#region entry.jsx
console.log(_jsx("div", {}), _jsx(_Fragment, { children: "fragment" }));

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,3 +1,4 @@
-function elem() {}
-function frag() {}
-console.log(elem("div", null), elem(frag, null, "fragment"));
+import {Fragment as _Fragment, jsx as _jsx} from "react/jsx-runtime";
+console.log(_jsx("div", {}), _jsx(_Fragment, {
+    children: "fragment"
+}));

```