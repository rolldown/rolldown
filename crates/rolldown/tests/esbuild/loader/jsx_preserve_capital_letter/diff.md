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

//#region entry.jsx
console.log(/* @__PURE__ */ jsx(mustStartWithUpperCaseLetter, {}));

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,6 +1,7 @@
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
+//#region entry.jsx
+console.log(/* @__PURE__ */ jsx(mustStartWithUpperCaseLetter, {}));

```