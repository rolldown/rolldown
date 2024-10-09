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
+++ rolldown	
@@ -1,7 +0,0 @@
-class Foo {
-    static t = {
-        get s() {
-            return 123;
-        }
-    };
-}

```