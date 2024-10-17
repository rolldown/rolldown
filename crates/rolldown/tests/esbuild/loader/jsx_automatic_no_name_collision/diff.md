# Reason
1. rolldown don't have `jsx.AutomaticRuntime` option
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

const { Link } = __toESM(require("@remix-run/react"));
const { createElement: _createElement } = __toESM(require("react"));

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
+var {Link} = __toESM(require("@remix-run/react"));
+var {createElement: _createElement} = __toESM(require("react"));
+var x = _createElement(Link, {
     ...y,
     key: z
 });

```