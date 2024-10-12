# Diff
## /out.js
### esbuild
```js
// entry.js
try {
  oldLocale = globalLocale._abbr;
  aliasedRequire = __require;
  aliasedRequire("./locale/" + name);
  getSetGlobalLocale(oldLocale);
} catch (e) {
}
var aliasedRequire;
```
### rolldown
```js

//#region entry.js
try {
	oldLocale = globalLocale._abbr;
	var aliasedRequire = require;
	aliasedRequire("./locale/" + name);
	getSetGlobalLocale(oldLocale);
} catch (e) {}

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,7 +1,6 @@
 try {
     oldLocale = globalLocale._abbr;
-    aliasedRequire = __require;
+    var aliasedRequire = require;
     aliasedRequire("./locale/" + name);
     getSetGlobalLocale(oldLocale);
 } catch (e) {}
-var aliasedRequire;

```