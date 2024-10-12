# Diff
## /out/entry.js
### esbuild
```js
// entry.js
for (y = void 0; !1; ) ;
var y;
for (z = 123; !1; ) ;
var z;
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	
@@ -1,4 +0,0 @@
-for (y = void 0; !1; ) ;
-var y;
-for (z = 123; !1; ) ;
-var z;

```