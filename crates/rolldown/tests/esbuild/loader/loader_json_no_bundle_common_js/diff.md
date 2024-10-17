# Reason
1. Wrong output
# Diff
## /out.js
### esbuild
```js
module.exports = { test: 123, "invalid-identifier": true };
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	
@@ -1,4 +0,0 @@
-module.exports = {
-    test: 123,
-    "invalid-identifier": true
-};

```