## /out.js
### esbuild
```js
var import_react = require("react");
var import_react2 = require("@remix-run/react");
const x = /* @__PURE__ */ (0, import_react.createElement)(import_react2.Link, { ...y, key: z });
```
### rolldown
```js
require("@remix-run/react");
require("react");

//#region entry.jsx
({ ...y }), z;

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,6 +1,5 @@
-var import_react = require("react");
-var import_react2 = require("@remix-run/react");
-const x = (0, import_react.createElement)(import_react2.Link, {
-    ...y,
-    key: z
-});
+require("@remix-run/react");
+require("react");
+({
+    ...y
+}, z);

```