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

//#region entry.js
console.log(
	// define wins over inject
	both === "define",
	bo.th === "defi.ne",
	// define forwards to inject
	first === "success (identifier)",
	fir.st === "success (dot name)"
);

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,3 +1,1 @@
-var second = "success (identifier)";
-var second2 = "success (dot name)";
-console.log(true, true, second === "success (identifier)", second2 === "success (dot name)");
+console.log(both === "define", bo.th === "defi.ne", first === "success (identifier)", fir.st === "success (dot name)");

```