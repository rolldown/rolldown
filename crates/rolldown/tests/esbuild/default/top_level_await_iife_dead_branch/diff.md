# Reason
1. should not appear `await`
# Diff
## /out.js
### esbuild
```js
(() => {
  // entry.js
  if (false) foo;
  if (false) for (foo of bar) ;
})();
```
### rolldown
```js
(function() {


//#region entry.js
if (false) await foo;
if (false) for await (foo of bar);

//#endregion
})();
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,4 +1,9 @@
-(() => {
-    if (false) foo;
-    if (false) for (foo of bar) ;
-})();
+(function() {
+
+
+//#region entry.js
+if (false) await foo;
+if (false) for await (foo of bar);
+
+//#endregion
+})();
\ No newline at end of file

```