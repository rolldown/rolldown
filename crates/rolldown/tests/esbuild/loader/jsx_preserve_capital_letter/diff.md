# Reason
1. rolldown don't have `jsx.Preserve` and `jsx.Parse` option
# Diff
## /out.js
### esbuild
```js
// foo.js
var MustStartWithUpperCaseLetter = class {
};

// entry.jsx
console.log(<MustStartWithUpperCaseLetter />);
```
### rolldown
```js
import { jsx } from "react/jsx-runtime";

//#region foo.js
var mustStartWithUpperCaseLetter = class {};

//#endregion
//#region entry.jsx
console.log(/* @__PURE__ */ jsx(mustStartWithUpperCaseLetter, {}));

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,6 +1,10 @@
-// foo.js
-var MustStartWithUpperCaseLetter = class {
-};
+import { jsx } from "react/jsx-runtime";
 
-// entry.jsx
-console.log(<MustStartWithUpperCaseLetter />);
\ No newline at end of file
+//#region foo.js
+var mustStartWithUpperCaseLetter = class {};
+
+//#endregion
+//#region entry.jsx
+console.log(/* @__PURE__ */ jsx(mustStartWithUpperCaseLetter, {}));
+
+//#endregion
\ No newline at end of file

```