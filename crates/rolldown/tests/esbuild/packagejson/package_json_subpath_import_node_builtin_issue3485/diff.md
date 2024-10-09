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

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	
@@ -1,4 +0,0 @@
-import fs from "fs";
-import http from "node:http";
-fs.readFileSync();
-http.createServer();

```