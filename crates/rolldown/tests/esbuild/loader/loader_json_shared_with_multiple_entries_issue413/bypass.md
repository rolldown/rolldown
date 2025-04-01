# Reason
1. rolldown auto code splitting for shared json module
# Diff
## /out/a.js
### esbuild
```js
// data.json
var data_default = { test: 123 };

// a.js
console.log("a:", data_default);
```
### rolldown
```js
import { data_default } from "./data.js";

//#region a.js
console.log("a:", data_default);
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/a.js
+++ rolldown	a.js
@@ -1,4 +1,2 @@
-var data_default = {
-    test: 123
-};
+import {data_default} from "./data.js";
 console.log("a:", data_default);

```
## /out/b.js
### esbuild
```js
// data.json
var data_default = { test: 123 };

// b.js
console.log("b:", data_default);
```
### rolldown
```js
import { data_default } from "./data.js";

//#region b.js
console.log("b:", data_default);
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/b.js
+++ rolldown	b.js
@@ -1,4 +1,2 @@
-var data_default = {
-    test: 123
-};
+import {data_default} from "./data.js";
 console.log("b:", data_default);

```