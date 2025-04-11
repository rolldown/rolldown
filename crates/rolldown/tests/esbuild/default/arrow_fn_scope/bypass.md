# Reason
1. generate same output as esbuild in `bundle` mode
# Diff
## /out.js
### esbuild
```js
// entry.js
tests = {
  0: (s = (e) => s + e, t) => s + t,
  1: (s, t = (e) => t + e) => t + s,
  2: (s = (a = (c) => s + a + c, b) => s + a + b, t, e) => s + t + e,
  3: (s, t, e = (a, b = (c) => e + b + c) => e + b + a) => e + s + t,
  4: (x = (s) => x + s, y, x + y),
  5: (y, x = (s) => x + s, x + y),
  6: (x = (s = (e) => x + s + e, t) => x + s + t, y, z, x + y + z),
  7: (y, z, x = (s, t = (e) => x + t + e) => x + t + s, x + y + z)
};
```
### rolldown
```js

//#region entry.js
tests = {
	0: (x = (y) => x + y, y) => x + y,
	1: (y, x = (y) => x + y) => x + y,
	2: (x = (y = (z) => x + y + z, z) => x + y + z, y, z) => x + y + z,
	3: (y, z, x = (z, y = (z) => x + y + z) => x + y + z) => x + y + z,
	4: (x = (y) => x + y, y, x + y),
	5: (y, x = (y) => x + y, x + y),
	6: (x = (y = (z) => x + y + z, z) => x + y + z, y, z, x + y + z),
	7: (y, z, x = (z, y = (z) => x + y + z) => x + y + z, x + y + z)
};

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,10 +1,10 @@
 tests = {
-    0: (s = e => s + e, t) => s + t,
-    1: (s, t = e => t + e) => t + s,
-    2: (s = (a = c => s + a + c, b) => s + a + b, t, e) => s + t + e,
-    3: (s, t, e = (a, b = c => e + b + c) => e + b + a) => e + s + t,
-    4: (x = s => x + s, y, x + y),
-    5: (y, x = s => x + s, x + y),
-    6: (x = (s = e => x + s + e, t) => x + s + t, y, z, x + y + z),
-    7: (y, z, x = (s, t = e => x + t + e) => x + t + s, x + y + z)
+    0: (x = y => x + y, y) => x + y,
+    1: (y, x = y => x + y) => x + y,
+    2: (x = (y = z => x + y + z, z) => x + y + z, y, z) => x + y + z,
+    3: (y, z, x = (z, y = z => x + y + z) => x + y + z) => x + y + z,
+    4: (x = y => x + y, y, x + y),
+    5: (y, x = y => x + y, x + y),
+    6: (x = (y = z => x + y + z, z) => x + y + z, y, z, x + y + z),
+    7: (y, z, x = (z, y = z => x + y + z) => x + y + z, x + y + z)
 };

```