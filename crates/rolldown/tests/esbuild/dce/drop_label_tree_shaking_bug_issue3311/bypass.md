# Reason
1. codegen sub optimal
2. oxc minifier will handle `EmptyStatement`
# Diff
## /out.js
### esbuild
```js
// entry.js
var myFunc = () => {
  console.log("keep");
};
var entry_default = myFunc;
export {
  entry_default as default
};
```
### rolldown
```js

//#region entry.js
const myFunc = () => {
	;
	console.log("keep");
};

//#endregion
export { myFunc as default };
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,5 +1,5 @@
 var myFunc = () => {
+    ;
     console.log("keep");
 };
-var entry_default = myFunc;
-export {entry_default as default};
+export {myFunc as default};

```