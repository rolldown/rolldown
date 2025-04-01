# Reason
1. could be done in minifier
# Diff
## /out.js
### esbuild
```js
a: {
  b: {
    if (x) break b;
    break a;
  }
}
a: {
  b: {
    if (x) break b;
    break a;
  }
}
a: {
  b: {
    if (x) break b;
    break a;
  }
}
```
### rolldown
```js

//#region entry.js
foo: bar: {
	if (x) break bar;
	break foo;
}
foo2: bar2: {
	if (x) break bar2;
	break foo2;
}
foo: bar: {
	if (x) break bar;
	break foo;
}
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,18 +1,12 @@
-a: {
-    b: {
-        if (x) break b;
-        break a;
-    }
+foo: bar: {
+    if (x) break bar;
+    break foo;
 }
-a: {
-    b: {
-        if (x) break b;
-        break a;
-    }
+foo2: bar2: {
+    if (x) break bar2;
+    break foo2;
 }
-a: {
-    b: {
-        if (x) break b;
-        break a;
-    }
+foo: bar: {
+    if (x) break bar;
+    break foo;
 }

```