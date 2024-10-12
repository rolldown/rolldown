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

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	
@@ -1,16 +0,0 @@
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
-console.log("correct");
-console.log("correct");

```