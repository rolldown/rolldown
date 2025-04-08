# Reason
1. could be done in minifier
# Diff
## /out.js
### esbuild
```js
// foo.js
var Y = class {
};

// entry.jsx
console.log(<Y tag-must-start-with-capital-letter />);
```
### rolldown
```js
import { jsx as _jsx } from "react/jsx-runtime";

//#region foo.js
var XYYYY = class {};

//#endregion
//#region entry.jsx
console.log(/* @__PURE__ */ _jsx(XYYYY, { "tag-must-start-with-capital-letter": true }));

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,6 +1,10 @@
-// foo.js
-var Y = class {
-};
+import { jsx as _jsx } from "react/jsx-runtime";
 
-// entry.jsx
-console.log(<Y tag-must-start-with-capital-letter />);
\ No newline at end of file
+//#region foo.js
+var XYYYY = class {};
+
+//#endregion
+//#region entry.jsx
+console.log(/* @__PURE__ */ _jsx(XYYYY, { "tag-must-start-with-capital-letter": true }));
+
+//#endregion
\ No newline at end of file

```