# Diff
## /out.js
### esbuild
```js
import * as ns from "./foo";
let foo = 234;
console.log(ns, ns.foo, foo);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	
@@ -1,3 +0,0 @@
-import * as ns from "./foo";
-let foo = 234;
-console.log(ns, ns.foo, foo);

```