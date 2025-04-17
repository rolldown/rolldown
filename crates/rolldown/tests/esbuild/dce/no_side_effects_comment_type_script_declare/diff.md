# Reason
1. rolldown should not shake the namespace iife
# Diff
## /out/entry.js
### esbuild
```js
var ns;
((ns2) => {
})(ns || (ns = {}));
```
### rolldown
```js

//#region entry.ts
let ns;
(function(_ns) {})(ns || (ns = {}));

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,2 +1,2 @@
 var ns;
-(ns2 => {})(ns || (ns = {}));
+(function (_ns) {})(ns || (ns = {}));

```