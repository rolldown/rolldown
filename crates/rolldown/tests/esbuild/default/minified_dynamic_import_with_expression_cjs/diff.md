# Diff
## /out.js
### esbuild
```js
import("foo");import(foo());
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	
@@ -1,2 +0,0 @@
-import("foo");
-import(foo());

```