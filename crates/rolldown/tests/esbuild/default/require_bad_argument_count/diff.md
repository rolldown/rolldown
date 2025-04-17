# Diff
## /out.js
### esbuild
```js
// entry.js
__require();
__require("a", "b");
try {
  __require();
  __require("a", "b");
} catch {
}
```
### rolldown
```js

//#region rolldown:runtime
var __require = /* @__PURE__ */ ((x) => typeof require !== "undefined" ? require : typeof Proxy !== "undefined" ? new Proxy(x, { get: (a, b) => (typeof require !== "undefined" ? require : a)[b] }) : x)(function(x) {
	if (typeof require !== "undefined") return require.apply(this, arguments);
	throw Error("Calling `require` for \"" + x + "\" in an environment that doesn't expose the `require` function.");
});

//#region entry.js
__require();
__require("a", "b");
try {
	__require();
	__require("a", "b");
} catch {}

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,4 +1,10 @@
+var __require = (x => typeof require !== "undefined" ? require : typeof Proxy !== "undefined" ? new Proxy(x, {
+    get: (a, b) => (typeof require !== "undefined" ? require : a)[b]
+}) : x)(function (x) {
+    if (typeof require !== "undefined") return require.apply(this, arguments);
+    throw Error("Calling `require` for \"" + x + "\" in an environment that doesn't expose the `require` function.");
+});
 __require();
 __require("a", "b");
 try {
     __require();

```