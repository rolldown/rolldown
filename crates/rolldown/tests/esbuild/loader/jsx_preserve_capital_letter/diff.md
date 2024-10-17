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
import { jsx as _jsx } from "react/jsx-runtime";

//#region foo.js
class mustStartWithUpperCaseLetter {}

//#endregion
//#region entry.jsx
console.log(_jsx(mustStartWithUpperCaseLetter, {}));

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
+import { jsx as _jsx } from "react/jsx-runtime";
 
-// entry.jsx
-console.log(<MustStartWithUpperCaseLetter />);
\ No newline at end of file
+//#region foo.js
+class mustStartWithUpperCaseLetter {}
+
+//#endregion
+//#region entry.jsx
+console.log(_jsx(mustStartWithUpperCaseLetter, {}));
+
+//#endregion
\ No newline at end of file

```