# Diff
## /out.js
### esbuild
```js
// pick-js.js
console.log("correct");

// pick-ts.ts
console.log("correct");

// pick-jsx.jsx
console.log("correct");

// pick-tsx.tsx
console.log("correct");

// order-js.ts
console.log("correct");

// order-jsx.ts
console.log("correct");

// node_modules/pkg/foo-js.ts
console.log("correct");

// node_modules/pkg/foo-jsx.tsx
console.log("correct");

// node_modules/pkg-exports/abc-js.ts
console.log("correct");

// node_modules/pkg-exports/abc-jsx.tsx
console.log("correct");

// node_modules/pkg-exports/lib/foo-js.ts
console.log("correct");

// node_modules/pkg-exports/lib/foo-jsx.tsx
console.log("correct");

// node_modules/pkg-imports/abc-js.ts
console.log("correct");

// node_modules/pkg-imports/abc-jsx.tsx
console.log("correct");

// node_modules/pkg-imports/lib/foo-js.ts
console.log("correct");

// node_modules/pkg-imports/lib/foo-jsx.tsx
console.log("correct");
```
### rolldown
```js
import "./pick-ts.js";
import "./pick-tsx.jsx";
import "./order-js.js";
import "./order-jsx.jsx";
import "pkg/foo-js.js";
import "pkg/foo-jsx.jsx";
import "pkg-exports/xyz-js";
import "pkg-exports/xyz-jsx";
import "pkg-exports/foo-js.js";
import "pkg-exports/foo-jsx.jsx";
import "#xyz-js";
import "#xyz-jsx";
import "#bar/foo-js.js";
import "#bar/foo-jsx.jsx";

//#region pick-js.js
console.log("correct");

//#endregion
//#region pick-jsx.jsx
console.log("correct");

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,16 +1,16 @@
+import "./pick-ts.js";
+import "./pick-tsx.jsx";
+import "./order-js.js";
+import "./order-jsx.jsx";
+import "pkg/foo-js.js";
+import "pkg/foo-jsx.jsx";
+import "pkg-exports/xyz-js";
+import "pkg-exports/xyz-jsx";
+import "pkg-exports/foo-js.js";
+import "pkg-exports/foo-jsx.jsx";
+import "#xyz-js";
+import "#xyz-jsx";
+import "#bar/foo-js.js";
+import "#bar/foo-jsx.jsx";
 console.log("correct");
 console.log("correct");
-console.log("correct");
-console.log("correct");
-console.log("correct");
-console.log("correct");
-console.log("correct");
-console.log("correct");
-console.log("correct");
-console.log("correct");
-console.log("correct");
-console.log("correct");
-console.log("correct");
-console.log("correct");
-console.log("correct");
-console.log("correct");

```