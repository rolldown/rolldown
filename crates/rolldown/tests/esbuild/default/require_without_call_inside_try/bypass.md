# Reason
1. esbuild will extract var decl, rolldown will not, this is trivial diff
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
// HIDDEN [rolldown:runtime]
//#region entry.js
try {
	oldLocale = globalLocale._abbr;
	var aliasedRequire = __require;
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
+    var aliasedRequire = __require;
     aliasedRequire("./locale/" + name);
     getSetGlobalLocale(oldLocale);
 } catch (e) {}
-var aliasedRequire;

```