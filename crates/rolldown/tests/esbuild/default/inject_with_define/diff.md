# Diff
## /out.js
### esbuild
```js
// inject.js
var second = "success (identifier)";
var second2 = "success (dot name)";

// entry.js
console.log(
  // define wins over inject
  true,
  true,
  // define forwards to inject
  second === "success (identifier)",
  second2 === "success (dot name)"
);
```
### rolldown
```js

//#region inject.js
let second$1 = "success (identifier)";
let second2 = "success (dot name)";

//#endregion
//#region entry.js
console.log(
	// define wins over inject
	true,
	true,
	// define forwards to inject
	second$1 === "success (identifier)",
	second2 === "success (dot name)"
);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,3 +1,3 @@
-var second = "success (identifier)";
+var second$1 = "success (identifier)";
 var second2 = "success (dot name)";
-console.log(true, true, second === "success (identifier)", second2 === "success (dot name)");
+console.log(true, true, second$1 === "success (identifier)", second2 === "success (dot name)");

```