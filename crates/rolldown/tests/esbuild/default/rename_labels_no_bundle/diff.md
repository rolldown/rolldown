# Diff
## /out.js
### esbuild
```js
foo: {
  bar: {
    if (x) break bar;
    break foo;
  }
}
foo2: {
  bar2: {
    if (x) break bar2;
    break foo2;
  }
}
foo: {
  bar: {
    if (x) break bar;
    break foo;
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
-foo: {
-    bar: {
-        if (x) break bar;
-        break foo;
-    }
+foo: bar: {
+    if (x) break bar;
+    break foo;
 }
-foo2: {
-    bar2: {
-        if (x) break bar2;
-        break foo2;
-    }
+foo2: bar2: {
+    if (x) break bar2;
+    break foo2;
 }
-foo: {
-    bar: {
-        if (x) break bar;
-        break foo;
-    }
+foo: bar: {
+    if (x) break bar;
+    break foo;
 }

```