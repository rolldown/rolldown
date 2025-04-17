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
	0: (x$1 = (y$2) => x$1 + y$2, y$1) => x$1 + y$1,
	1: (y$1, x$1 = (y$2) => x$1 + y$2) => x$1 + y$1,
	2: (x$1 = (y$2 = (z$3) => x$1 + y$2 + z$3, z$2) => x$1 + y$2 + z$2, y$1, z$1) => x$1 + y$1 + z$1,
	3: (y$1, z$1, x$1 = (z$2, y$2 = (z$3) => x$1 + y$2 + z$3) => x$1 + y$2 + z$2) => x$1 + y$1 + z$1,
	4: (x = (y$1) => x + y$1, y, x + y),
	5: (y, x = (y$1) => x + y$1, x + y),
	6: (x = (y$1 = (z$2) => x + y$1 + z$2, z$1) => x + y$1 + z$1, y, z, x + y + z),
	7: (y, z, x = (z$1, y$1 = (z$2) => x + y$1 + z$2) => x + y$1 + z$1, x + y + z)
};

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
+    0: (x$1 = y$2 => x$1 + y$2, y$1) => x$1 + y$1,
+    1: (y$1, x$1 = y$2 => x$1 + y$2) => x$1 + y$1,
+    2: (x$1 = (y$2 = z$3 => x$1 + y$2 + z$3, z$2) => x$1 + y$2 + z$2, y$1, z$1) => x$1 + y$1 + z$1,
+    3: (y$1, z$1, x$1 = (z$2, y$2 = z$3 => x$1 + y$2 + z$3) => x$1 + y$2 + z$2) => x$1 + y$1 + z$1,
+    4: (x = y$1 => x + y$1, y, x + y),
+    5: (y, x = y$1 => x + y$1, x + y),
+    6: (x = (y$1 = z$2 => x + y$1 + z$2, z$1) => x + y$1 + z$1, y, z, x + y + z),
+    7: (y, z, x = (z$1, y$1 = z$2 => x + y$1 + z$2) => x + y$1 + z$1, x + y + z)
 };

```