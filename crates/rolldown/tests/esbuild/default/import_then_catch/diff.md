# Diff
## /out.js
### esbuild
```js
// entry.js
import(name).then(pass, fail);
import(name).then(pass).catch(fail);
import(name).catch(fail);
```
### rolldown
```js


```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,3 +0,0 @@
-import(name).then(pass, fail);
-import(name).then(pass).catch(fail);
-import(name).catch(fail);

```