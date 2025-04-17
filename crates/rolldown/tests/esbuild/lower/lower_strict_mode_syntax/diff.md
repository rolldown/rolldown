# Diff
## /out.js
### esbuild
```js
// for-in.js
if (test) {
  a = b;
  for (a in {}) ;
}
var a;
x = y;
for (x in {}) ;
var x;
```
### rolldown
```js

//#region for-in.js
if (test) for (var a = b in {});
for (var x = y in {});

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,8 +1,4 @@
-if (test) {
-    a = b;
-    for (a in {}) ;
-}
-var a;
-x = y;
-for (x in {}) ;
-var x;
+
+//#region for-in.js
+if (test) for (var a = b in {});
+for (var x = y in {});

```