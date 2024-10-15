# Reason
1. sub optimal
# Diff
## /out.js
### esbuild
```js
// entry.js
import * as fs from "fs";
import defaultValue from "fs";
import { readFileSync } from "fs";
console.log(fs, readFileSync, defaultValue);
```
### rolldown
```js
import * as fs from "node:fs";
import defaultValue, { readFileSync } from "node:fs";

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
-import * as fs from "fs";
-import defaultValue from "fs";
-import {readFileSync} from "fs";
+import * as fs from "node:fs";
+import defaultValue, {readFileSync} from "node:fs";
 console.log(fs, readFileSync, defaultValue);

```