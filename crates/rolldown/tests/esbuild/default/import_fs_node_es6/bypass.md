# Reason
1. rolldown merge two import stmt
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
import defaultValue, { readFileSync } from "fs";

//#region entry.js
console.log(fs, readFileSync, defaultValue);

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,4 +1,3 @@
 import * as fs from "fs";
-import defaultValue from "fs";
-import {readFileSync} from "fs";
+import defaultValue, {readFileSync} from "fs";
 console.log(fs, readFileSync, defaultValue);

```