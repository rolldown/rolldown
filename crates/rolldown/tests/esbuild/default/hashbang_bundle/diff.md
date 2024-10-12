# Diff
## /out.js
### esbuild
```js
#!/usr/bin/env a

// code.js
var code = 0;

// entry.js
process.exit(code);
```
### rolldown
```js

//#region code.js
#!/usr/bin/env b
const code = 0;

//#endregion
//#region entry.js
#!/usr/bin/env a
process.exit(code);

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,2 +1,11 @@
-var code = 0;
+
+//#region code.js
+#!/usr/bin/env b
+const code = 0;
+
+//#endregion
+//#region entry.js
+#!/usr/bin/env a
 process.exit(code);
+
+//#endregion

```