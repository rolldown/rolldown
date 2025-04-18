# Reason
1. should rename `require` when it is appear in param position
# Diff
## /out.js
### esbuild
```js
// entry.js
((require2) => require2("/test.txt"))();
```
### rolldown
```js
//#region entry.js
((require) => require("/test.txt"))();

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,1 +1,1 @@
-(require2 => require2("/test.txt"))();
+(require => require("/test.txt"))();

```