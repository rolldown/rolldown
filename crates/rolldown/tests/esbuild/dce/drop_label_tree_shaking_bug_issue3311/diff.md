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
	DROP: {
		console.log("drop");
	}
	console.log("keep");
};
var entry_default = myFunc;

//#endregion
export { entry_default as default };

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,5 +1,8 @@
 var myFunc = () => {
+    DROP: {
+        console.log("drop");
+    }
     console.log("keep");
 };
 var entry_default = myFunc;
 export {entry_default as default};

```