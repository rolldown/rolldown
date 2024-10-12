# Diff
## /out.js
### esbuild
```js
// entry.js
import fs from "fs";
import http from "node:http";
fs.readFileSync();
http.createServer();
```
### rolldown
```js
import { default as fs } from "node:fs";
import { default as http } from "node:http";

//#region entry.js
fs.readFileSync();
http.createServer();

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,4 +1,4 @@
-import fs from "fs";
-import http from "node:http";
+import {default as fs} from "node:fs";
+import {default as http} from "node:http";
 fs.readFileSync();
 http.createServer();

```