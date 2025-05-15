# Reason
1. rolldown merge two import stmt
# Diff
## /out.js
### esbuild
```js
// entry.js
var fs = __toESM(require("fs"));
var import_fs = __toESM(require("fs"));
var import_fs2 = require("fs");
console.log(fs, import_fs2.readFileSync, import_fs.default);
```
### rolldown
```js

const fs = __toESM(require("fs"));

//#region entry.js
console.log(fs, fs.readFileSync, fs.default);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,4 +1,2 @@
 var fs = __toESM(require("fs"));
-var import_fs = __toESM(require("fs"));
-var import_fs2 = require("fs");
-console.log(fs, import_fs2.readFileSync, import_fs.default);
+console.log(fs, fs.readFileSync, fs.default);

```