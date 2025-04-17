# Reason
1. Could be done in minifier
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

//#region entry.js
function empty() {}
function id(x) {
	return x;
}
var y = empty();
var z = id(123);

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,4 +1,6 @@
-for (y = void 0; !1; ) ;
-var y;
-for (z = 123; !1; ) ;
-var z;
+function empty() {}
+function id(x) {
+    return x;
+}
+var y = empty();
+var z = id(123);

```