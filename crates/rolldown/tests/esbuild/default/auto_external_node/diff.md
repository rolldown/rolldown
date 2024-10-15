# Reason
1. should not transform `{default as fs}`
2. `node:path` is side effects free
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
import fs from "node:fs/promises";
import "node:path";
import "node:what-is-this";

//#region entry.js
fs.readFile();

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,3 +1,4 @@
 import fs from "node:fs/promises";
+import "node:path";
 import "node:what-is-this";
 fs.readFile();

```