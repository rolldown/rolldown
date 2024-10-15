# Reason
1. should not rewrite `fs` to `node:fs`
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
"use strict";

const fs = __toESM(require("node:fs"));
const { default: defaultValue, readFileSync } = __toESM(require("node:fs"));

//#region entry.js
console.log(fs, readFileSync, defaultValue);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,4 +1,3 @@
-var fs = __toESM(require("fs"));
-var import_fs = __toESM(require("fs"));
-var import_fs2 = require("fs");
-console.log(fs, import_fs2.readFileSync, import_fs.default);
+var fs = __toESM(require("node:fs"));
+var {default: defaultValue, readFileSync} = __toESM(require("node:fs"));
+console.log(fs, readFileSync, defaultValue);

```