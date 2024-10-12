# Diff
## /out.js
### esbuild
```js
// entry.ts
console.log(`
					SameFile.STR = str 1
					SameFile.NUM = 123
					CrossFile.STR = str 2
					CrossFile.NUM = 321
				`);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	
@@ -1,6 +0,0 @@
-console.log(`
-					SameFile.STR = str 1
-					SameFile.NUM = 123
-					CrossFile.STR = str 2
-					CrossFile.NUM = 321
-				`);

```