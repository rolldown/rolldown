# Diff
## /out/foo-FYKHFNL2.copy
### esbuild
```js
{}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/foo-FYKHFNL2.copy
+++ rolldown	
@@ -1,1 +0,0 @@
-{}

```
## /out/js-entry.js
### esbuild
```js
// foo.json
var foo_default = {};

// js-entry.js
import copy from "./foo-FYKHFNL2.copy" assert { type: "json" };
use(foo_default, copy, foo_default, void 0);
export {
  foo_default as default
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/js-entry.js
+++ rolldown	
@@ -1,9 +0,0 @@
-// foo.json
-var foo_default = {};
-
-// js-entry.js
-import copy from "./foo-FYKHFNL2.copy" assert { type: "json" };
-use(foo_default, copy, foo_default, void 0);
-export {
-  foo_default as default
-};
\ No newline at end of file

```
## /out/ts-entry.js
### esbuild
```js
// foo.json
var foo_default = {};

// ts-entry.ts
import copy from "./foo-FYKHFNL2.copy" assert { type: "json" };
use(foo_default, copy, foo_default, void 0);
export {
  foo_default as default
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/ts-entry.js
+++ rolldown	
@@ -1,9 +0,0 @@
-// foo.json
-var foo_default = {};
-
-// ts-entry.ts
-import copy from "./foo-FYKHFNL2.copy" assert { type: "json" };
-use(foo_default, copy, foo_default, void 0);
-export {
-  foo_default as default
-};
\ No newline at end of file

```