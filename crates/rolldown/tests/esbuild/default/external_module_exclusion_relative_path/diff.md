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

```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out/index.js
+++ rolldown	
@@ -1,5 +0,0 @@
-import foo from "../src/nested/folder/foo.js";
-import out from "./in-out-dir.js";
-import sha256 from "../src/sha256.min.js";
-import config from "/api/config?a=1&b=2";
-console.log(foo, out, sha256, config);

```