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
import { Link } from "@remix-run/react";
import { createElement as _createElement } from "react";

//#region entry.jsx
const x = _createElement(Link, {
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
+import {Link} from "@remix-run/react";
+import {createElement as _createElement} from "react";
+var x = _createElement(Link, {
     ...y,
     key: z
 });

```