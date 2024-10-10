## /out/a.js
### esbuild
```js
import {
  foo,
  init_a
} from "./chunk-PDZFCFBH.js";
init_a();
export {
  foo
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/a.js
+++ rolldown	
@@ -1,3 +0,0 @@
-import {foo, init_a} from "./chunk-PDZFCFBH.js";
-init_a();
-export {foo};

```
## /out/b.js
### esbuild
```js
import {
  __toCommonJS,
  a_exports,
  init_a
} from "./chunk-PDZFCFBH.js";

// b.js
var bar = (init_a(), __toCommonJS(a_exports));
export {
  bar
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/b.js
+++ rolldown	
@@ -1,3 +0,0 @@
-import {__toCommonJS, a_exports, init_a} from "./chunk-PDZFCFBH.js";
-var bar = (init_a(), __toCommonJS(a_exports));
-export {bar};

```
# Diff
## /out/a.js
### esbuild
```js
import {
  foo,
  init_a
} from "./chunk-PDZFCFBH.js";
init_a();
export {
  foo
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/a.js
+++ rolldown	
@@ -1,3 +0,0 @@
-import {foo, init_a} from "./chunk-PDZFCFBH.js";
-init_a();
-export {foo};

```
## /out/b.js
### esbuild
```js
import {
  __toCommonJS,
  a_exports,
  init_a
} from "./chunk-PDZFCFBH.js";

// b.js
var bar = (init_a(), __toCommonJS(a_exports));
export {
  bar
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/b.js
+++ rolldown	
@@ -1,3 +0,0 @@
-import {__toCommonJS, a_exports, init_a} from "./chunk-PDZFCFBH.js";
-var bar = (init_a(), __toCommonJS(a_exports));
-export {bar};

```