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
import "pkg-exports/foo-js.js";
import "pkg-exports/foo-jsx.jsx";
import "#bar/foo-js.js";
import "#bar/foo-jsx.jsx";

//#region pick-js.js
console.log("correct");

//#endregion
//#region pick-ts.ts
console.log("correct");

//#endregion
//#region pick-jsx.jsx
console.log("correct");

//#endregion
//#region pick-tsx.tsx
console.log("correct");

//#endregion
//#region order-js.ts
console.log("correct");

//#endregion
//#region order-jsx.ts
console.log("correct");

//#endregion
//#region node_modules/pkg/foo-js.ts
console.log("correct");

//#endregion
//#region node_modules/pkg/foo-jsx.tsx
console.log("correct");

//#endregion
//#region node_modules/pkg-exports/abc-js.ts
console.log("correct");

//#endregion
//#region node_modules/pkg-exports/abc-jsx.tsx
console.log("correct");

//#endregion
//#region node_modules/pkg-imports/abc-js.ts
console.log("correct");

//#endregion
//#region node_modules/pkg-imports/abc-jsx.tsx
console.log("correct");

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,4 +1,8 @@
+import "pkg-exports/foo-js.js";
+import "pkg-exports/foo-jsx.jsx";
+import "#bar/foo-js.js";
+import "#bar/foo-jsx.jsx";
 console.log("correct");
 console.log("correct");
 console.log("correct");
 console.log("correct");
@@ -9,8 +13,4 @@
 console.log("correct");
 console.log("correct");
 console.log("correct");
 console.log("correct");
-console.log("correct");
-console.log("correct");
-console.log("correct");
-console.log("correct");

```