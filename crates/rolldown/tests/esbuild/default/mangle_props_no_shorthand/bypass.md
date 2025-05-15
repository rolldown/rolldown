# Reason
1. could be done in minifier
# Diff
## /out.js
### esbuild
```js
export let yyyyy = ({ y }) => ({ y: y });
```
### rolldown
```js
//#region entry.js
// This should print as "({ y }) => ({ y: y })" not "({ y: y }) => ({ y: y })"
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
-    y: y
+var yyyyy = ({xxxxx}) => ({
+    xxxxx
 });
+export {yyyyy};

```