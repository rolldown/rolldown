# Diff
## /out.js
### esbuild
```js
#!/usr/bin/env node
process.exit(0);
```
### rolldown
```js

//#region entry.js
#!/usr/bin/env node
process.exit(0);

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,1 +1,6 @@
+
+//#region entry.js
+#!/usr/bin/env node
 process.exit(0);
+
+//#endregion

```