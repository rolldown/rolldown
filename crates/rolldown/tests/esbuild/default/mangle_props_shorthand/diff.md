# Diff
## /out.js
### esbuild
```js
export let yyyyy = ({ y }) => ({ y });
```
### rolldown
```js

//#region entry.js
let yyyyy = ({ xxxxx }) => ({ xxxxx });

//#endregion
export { yyyyy };

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,3 +1,4 @@
-export let yyyyy = ({y}) => ({
-    y
+var yyyyy = ({xxxxx}) => ({
+    xxxxx
 });
+export {yyyyy};

```