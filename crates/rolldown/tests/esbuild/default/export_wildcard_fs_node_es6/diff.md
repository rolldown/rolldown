# Reason
1. two `import` statement are redundant
# Diff
## /out.js
### esbuild
```js
// entry.js
export * from "fs";

// internal.js
var foo = 123;

// entry.js
export * from "./external";
export {
  foo
};
```
### rolldown
```js
import "fs";
import "./external";

export * from "fs"

export * from "./external"

//#region internal.js
let foo = 123;

//#endregion
export { foo };
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,4 +1,6 @@
+import "fs";
+import "./external";
 export * from "fs";
-var foo = 123;
 export * from "./external";
+var foo = 123;
 export {foo};

```