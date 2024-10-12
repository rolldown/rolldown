# Diff
## /out.js
### esbuild
```js
// entry.js
console.log(/* @__PURE__ */ React.createElement("div", null));
```
### rolldown
```js
import { jsx as _jsx } from "react/jsx-runtime";

//#region entry.js
console.log(_jsx("div", {}));

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,1 +1,2 @@
-console.log(React.createElement("div", null));
+import {jsx as _jsx} from "react/jsx-runtime";
+console.log(_jsx("div", {}));

```