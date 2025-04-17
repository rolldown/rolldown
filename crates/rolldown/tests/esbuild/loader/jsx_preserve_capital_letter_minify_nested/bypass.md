# Reason
1. could be done in minifier
# Diff
## /out.js
### esbuild
```js
// entry.jsx
x = () => {
  class Y {
  }
  return <Y tag-must-start-with-capital-letter />;
};
```
### rolldown
```js
import { jsx } from "react/jsx-runtime";

//#region entry.jsx
x = () => {
	class XYYYYY {}
	return /* @__PURE__ */ jsx(XYYYYY, { "tag-must-start-with-capital-letter": true });
};

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,6 +1,7 @@
-// entry.jsx
+import { jsx } from "react/jsx-runtime";
+
+//#region entry.jsx
 x = () => {
-  class Y {
-  }
-  return <Y tag-must-start-with-capital-letter />;
-};
\ No newline at end of file
+	class XYYYYY {}
+	return /* @__PURE__ */ jsx(XYYYYY, { "tag-must-start-with-capital-letter": true });
+};

```