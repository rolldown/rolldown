# Diff
## /out.js
### esbuild
```js
// custom-react.js
var require_custom_react = __commonJS({
  "custom-react.js"(exports, module) {
    module.exports = {};
  }
});

// entry.jsx
var import_custom_react = __toESM(require_custom_react());
console.log(/* @__PURE__ */ (0, import_custom_react.elem)("div", null), /* @__PURE__ */ (0, import_custom_react.elem)(import_custom_react.frag, null, "fragment"));
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
@@ -1,7 +1,4 @@
-var require_custom_react = __commonJS({
-    "custom-react.js"(exports, module) {
-        module.exports = {};
-    }
-});
-var import_custom_react = __toESM(require_custom_react());
-console.log((0, import_custom_react.elem)("div", null), (0, import_custom_react.elem)(import_custom_react.frag, null, "fragment"));
+import {Fragment as _Fragment, jsx as _jsx} from "react/jsx-runtime";
+console.log(_jsx("div", {}), _jsx(_Fragment, {
+    children: "fragment"
+}));

```