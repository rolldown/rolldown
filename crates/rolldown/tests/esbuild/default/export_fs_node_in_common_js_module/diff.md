# Diff
## /out.js
### esbuild
```js
// entry.js
import * as fs from "fs";
import { readFileSync } from "fs";
var require_entry = __commonJS({
  "entry.js"(exports) {
    exports.fs = fs;
    exports.readFileSync = readFileSync;
    exports.foo = 123;
  }
});
export default require_entry();
```
### rolldown
```js
import * as fs$1 from "fs";
import { readFileSync } from "fs";


//#region entry.js
var require_entry = __commonJS({ "entry.js"(exports) {
	exports.fs = fs$1;
	exports.readFileSync = readFileSync;
	exports.foo = 123;
} });

//#endregion
export default require_entry();

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,9 +1,9 @@
-import * as fs from "fs";
+import * as fs$1 from "fs";
 import {readFileSync} from "fs";
 var require_entry = __commonJS({
     "entry.js"(exports) {
-        exports.fs = fs;
+        exports.fs = fs$1;
         exports.readFileSync = readFileSync;
         exports.foo = 123;
     }
 });

```