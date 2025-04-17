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

//#region rolldown:runtime
var __require = /* @__PURE__ */ ((x) => typeof require !== "undefined" ? require : typeof Proxy !== "undefined" ? new Proxy(x, { get: (a, b) => (typeof require !== "undefined" ? require : a)[b] }) : x)(function(x) {
	if (typeof require !== "undefined") return require.apply(this, arguments);
	throw Error("Calling `require` for \"" + x + "\" in an environment that doesn't expose the `require` function.");
});

//#region entry.js
try {
	oldLocale = globalLocale._abbr;
	var aliasedRequire = __require;
	aliasedRequire("./locale/" + name);
	getSetGlobalLocale(oldLocale);
} catch (e) {}

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,7 +1,12 @@
+var __require = (x => typeof require !== "undefined" ? require : typeof Proxy !== "undefined" ? new Proxy(x, {
+    get: (a, b) => (typeof require !== "undefined" ? require : a)[b]
+}) : x)(function (x) {
+    if (typeof require !== "undefined") return require.apply(this, arguments);
+    throw Error("Calling `require` for \"" + x + "\" in an environment that doesn't expose the `require` function.");
+});
 try {
     oldLocale = globalLocale._abbr;
-    aliasedRequire = __require;
+    var aliasedRequire = __require;
     aliasedRequire("./locale/" + name);
     getSetGlobalLocale(oldLocale);
 } catch (e) {}
-var aliasedRequire;

```