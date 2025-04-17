# Reason
1. cjs module lexer can't recognize esbuild interop pattern
# Diff
## /out.js
### esbuild
```js
"use strict";

// entry.js
var entry_exports = {};
__export(entry_exports, {
  a: () => a
});
module.exports = __toCommonJS(entry_exports);
var a = 1;
```
### rolldown
```js
"use strict";

//#region entry.js
let a = 1;

exports.a = a
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,7 +1,2 @@
-"use strict";
-var entry_exports = {};
-__export(entry_exports, {
-    a: () => a
-});
-module.exports = __toCommonJS(entry_exports);
 var a = 1;
+exports.a = a;

```