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
import { jsx } from "react/jsx-runtime";

//#region foo.js
var mustStartWithUpperCaseLetter = class {};

//#region entry.jsx
console.log(/* @__PURE__ */ jsx(mustStartWithUpperCaseLetter, { "tag-must-start-with-capital-letter": true }));

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,6 +1,7 @@
-// foo.js
-var Y = class {
-};
+import { jsx } from "react/jsx-runtime";
 
-// entry.jsx
-console.log(<Y tag-must-start-with-capital-letter />);
\ No newline at end of file
+//#region foo.js
+var mustStartWithUpperCaseLetter = class {};
+
+//#region entry.jsx
+console.log(/* @__PURE__ */ jsx(mustStartWithUpperCaseLetter, { "tag-must-start-with-capital-letter": true }));

```