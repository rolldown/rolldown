# Diff
## /out/cjs-in-esm.js
### esbuild
```js
export let foo = 1;
exports.foo = 2;
module.exports = 3;
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/cjs-in-esm.js
+++ rolldown	
@@ -1,3 +0,0 @@
-export let foo = 1;
-exports.foo = 2;
-module.exports = 3;

```
## /out/import-in-cjs.js
### esbuild
```js
import { foo } from "bar";
exports.foo = foo;
module.exports = foo;
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/import-in-cjs.js
+++ rolldown	
@@ -1,3 +0,0 @@
-import {foo} from "bar";
-exports.foo = foo;
-module.exports = foo;

```
## /out/no-warnings-here.js
### esbuild
```js
console.log(module, exports);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/no-warnings-here.js
+++ rolldown	
@@ -1,1 +0,0 @@
-console.log(module, exports);

```