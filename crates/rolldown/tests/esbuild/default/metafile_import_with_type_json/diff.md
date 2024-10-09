# Diff
## /out/entry.js
### esbuild
```js
// project/data.json
var data_default = { some: "data" };

// project/data.json with { type: 'json' }
var data_default2 = { some: "data" };

// project/entry.js
x = [data_default, data_default, data_default2];
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
-var data_default = {
-    some: "data"
-};
-var data_default2 = {
-    some: "data"
-};
-x = [data_default, data_default, data_default2];

```