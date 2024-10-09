# Diff
## /out/entry.js
### esbuild
```js
// entry.js
import fs from "node:fs/promises";
import "node:what-is-this";
fs.readFile();
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	
@@ -1,3 +0,0 @@
-import fs from "node:fs/promises";
-import "node:what-is-this";
-fs.readFile();

```