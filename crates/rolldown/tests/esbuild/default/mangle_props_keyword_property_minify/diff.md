# Reason
1. could be done in minifier
# Diff
## /out/entry.js
### esbuild
```js
class Foo{static t={get s(){return 123}}}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,7 +0,0 @@
-class Foo {
-    static t = {
-        get s() {
-            return 123;
-        }
-    };
-}

```