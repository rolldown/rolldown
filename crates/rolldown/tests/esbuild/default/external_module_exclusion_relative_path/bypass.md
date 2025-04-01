# Reason
1. different file system
# Diff
## /Users/user/project/out/index.js
### esbuild
```js
// Users/user/project/src/nested/folder/test.js
import foo from "../src/nested/folder/foo.js";
import out from "./in-out-dir.js";
import sha256 from "../src/sha256.min.js";
import config from "/api/config?a=1&b=2";
console.log(foo, out, sha256, config);
```
### rolldown
```js
import foo from "./nested/folder/foo.js";
import out from "../out/in-out-dir.js";
import sha256 from "./sha256.min.js";
import config from "/api/config?a=1&b=2";

//#region nested/folder/test.js
console.log(foo, out, sha256, config);
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out/index.js
+++ rolldown	index.js
@@ -1,5 +1,5 @@
-import foo from "../src/nested/folder/foo.js";
-import out from "./in-out-dir.js";
-import sha256 from "../src/sha256.min.js";
+import foo from "./nested/folder/foo.js";
+import out from "../out/in-out-dir.js";
+import sha256 from "./sha256.min.js";
 import config from "/api/config?a=1&b=2";
 console.log(foo, out, sha256, config);

```