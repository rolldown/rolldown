# Diff
## /out/entry.js
### esbuild
```js
// entry.js
import a from "data:some/thing;what,someData%31%32%33";
import b from "data:other/thing;stuff;base64,c29tZURhdGEyMzQ=";
console.log(a, b);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	
@@ -1,3 +0,0 @@
-import a from "data:some/thing;what,someData%31%32%33";
-import b from "data:other/thing;stuff;base64,c29tZURhdGEyMzQ=";
-console.log(a, b);

```