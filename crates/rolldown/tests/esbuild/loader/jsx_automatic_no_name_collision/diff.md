# Reason
1. esbuild did not needs `__toESM`
# Diff
## /out.js
### esbuild
```js
var import_react = require("react");
var import_react2 = require("@remix-run/react");
const x = /* @__PURE__ */ (0, import_react.createElement)(import_react2.Link, { ...y, key: z });
```
### rolldown
```js
"use strict";


const __remix_run_react = __toESM(require("@remix-run/react"));
const react = __toESM(require("react"));

//#region entry.jsx
const x = /* @__PURE__ */ (0, react.createElement)(__remix_run_react.Link, {
	...y,
	key: z
});
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,6 +1,6 @@
-var import_react = require("react");
-var import_react2 = require("@remix-run/react");
-const x = (0, import_react.createElement)(import_react2.Link, {
+var __remix_run_react = __toESM(require("@remix-run/react"));
+var react = __toESM(require("react"));
+var x = (0, react.createElement)(__remix_run_react.Link, {
     ...y,
     key: z
 });

```