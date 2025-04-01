# Reason
1. different chunk naming style
# Diff
## /out/entry.js
### esbuild
```js
// entry.js
import("./foo-X6C7FV5C.js").then(({ default: { bar } }) => console.log(bar));
```
### rolldown
```js

//#region entry.js
import("./foo.js").then(({ default: { bar } }) => console.log(bar));
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,1 +1,1 @@
-import("./foo-X6C7FV5C.js").then(({default: {bar}}) => console.log(bar));
+import("./foo.js").then(({default: {bar}}) => console.log(bar));

```