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
import { jsx as _jsx } from "react/jsx-runtime";

//#region entry.jsx
x = () => {
	class XYYYYY {}
	return _jsx(
		XYYYYY,
		// This should be named "Y" due to frequency analysis
		{ "tag-must-start-with-capital-letter": true }
);
};

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,6 +1,13 @@
-// entry.jsx
+import { jsx as _jsx } from "react/jsx-runtime";
+
+//#region entry.jsx
 x = () => {
-  class Y {
-  }
-  return <Y tag-must-start-with-capital-letter />;
-};
\ No newline at end of file
+	class XYYYYY {}
+	return _jsx(
+		XYYYYY,
+		// This should be named "Y" due to frequency analysis
+		{ "tag-must-start-with-capital-letter": true }
+);
+};
+
+//#endregion

```