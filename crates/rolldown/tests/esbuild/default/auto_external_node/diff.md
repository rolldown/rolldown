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
import { default as fs } from "node:fs/promises";
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
-import fs from "node:fs/promises";
+import {default as fs} from "node:fs/promises";
+import "node:path";
 import "node:what-is-this";
 fs.readFile();

```