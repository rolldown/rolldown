# Reason
1. oxc constant folding
# Diff
## /out/entry.js
### esbuild
```js
// <data:text/javascript,console.log(1+2)>
console.log(1 + 2);
```
### rolldown
```js

//#region <data:text/javascript,console.log(1+2)>
console.log(3);

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,1 +1,1 @@
-console.log(1 + 2);
+console.log(3);

```