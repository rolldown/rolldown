# Reason
1. not support copy loader
# Diff
## /out/inject-IFR6YGWW.js
### esbuild
```js
console.log('in inject.js')
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/inject-IFR6YGWW.js
+++ rolldown	
@@ -1,1 +0,0 @@
-console.log('in inject.js');

```
## /out/entry.js
### esbuild
```js
// src/entry.ts
import "./inject-IFR6YGWW.js";
console.log("in entry.ts");
```
### rolldown
```js

//#region entry.ts
console.log("in entry.ts");
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,2 +1,1 @@
-import "./inject-IFR6YGWW.js";
 console.log("in entry.ts");

```