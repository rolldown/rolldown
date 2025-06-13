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

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	
@@ -1,2 +0,0 @@
-var ns;
-(ns2 => {})(ns || (ns = {}));

```