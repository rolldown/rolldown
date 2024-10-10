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
import * as fs from "fs";
import { default as defaultValue, readFileSync } from "fs";

//#region entry.js
console.log(fs, readFileSync, defaultValue);

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry_js.js
@@ -1,4 +1,3 @@
 import * as fs from "fs";
-import defaultValue from "fs";
-import {readFileSync} from "fs";
+import {default as defaultValue, readFileSync} from "fs";
 console.log(fs, readFileSync, defaultValue);

```