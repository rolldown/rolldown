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
## /out/chunk-PDZFCFBH.js
### esbuild
```js
// a.js
var a_exports = {};
__export(a_exports, {
  foo: () => foo
});
var foo;
var init_a = __esm({
  "a.js"() {
  }
});

export {
  __toCommonJS,
  foo,
  a_exports,
  init_a
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/chunk-PDZFCFBH.js
+++ rolldown	
@@ -1,17 +0,0 @@
-// a.js
-var a_exports = {};
-__export(a_exports, {
-  foo: () => foo
-});
-var foo;
-var init_a = __esm({
-  "a.js"() {
-  }
-});
-
-export {
-  __toCommonJS,
-  foo,
-  a_exports,
-  init_a
-};
\ No newline at end of file

```