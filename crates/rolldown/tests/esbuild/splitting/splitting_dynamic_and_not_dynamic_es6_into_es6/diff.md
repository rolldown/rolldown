## /out/entry.js
### esbuild
```js
import {
  bar
} from "./chunk-UDDKLWVZ.js";

// entry.js
import("./foo-BNHN4WV6.js").then(({ bar: b }) => console.log(bar, b));
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
-import {bar} from "./chunk-UDDKLWVZ.js";
-import("./foo-BNHN4WV6.js").then(({bar: b}) => console.log(bar, b));

```
## /out/foo-BNHN4WV6.js
### esbuild
```js
import {
  bar
} from "./chunk-UDDKLWVZ.js";
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
--- esbuild	/out/foo-BNHN4WV6.js
+++ rolldown	
@@ -1,2 +0,0 @@
-import {bar} from "./chunk-UDDKLWVZ.js";
-export {bar};

```
## /out/chunk-UDDKLWVZ.js
### esbuild
```js
// foo.js
var bar = 123;

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
--- esbuild	/out/chunk-UDDKLWVZ.js
+++ rolldown	
@@ -1,2 +0,0 @@
-var bar = 123;
-export {bar};

```
# Diff
## /out/entry.js
### esbuild
```js
import {
  bar
} from "./chunk-UDDKLWVZ.js";

// entry.js
import("./foo-BNHN4WV6.js").then(({ bar: b }) => console.log(bar, b));
```
### rolldown
```js
import { bar } from "./foo2.js";

//#region entry.js
import("./foo.js").then(({ bar: b }) => console.log(bar, b));

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,2 +1,2 @@
-import {bar} from "./chunk-UDDKLWVZ.js";
-import("./foo-BNHN4WV6.js").then(({bar: b}) => console.log(bar, b));
+import {bar} from "./foo2.js";
+import("./foo.js").then(({bar: b}) => console.log(bar, b));

```
## /out/foo-BNHN4WV6.js
### esbuild
```js
import {
  bar
} from "./chunk-UDDKLWVZ.js";
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
--- esbuild	/out/foo-BNHN4WV6.js
+++ rolldown	
@@ -1,2 +0,0 @@
-import {bar} from "./chunk-UDDKLWVZ.js";
-export {bar};

```
## /out/chunk-UDDKLWVZ.js
### esbuild
```js
// foo.js
var bar = 123;

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
--- esbuild	/out/chunk-UDDKLWVZ.js
+++ rolldown	
@@ -1,2 +0,0 @@
-var bar = 123;
-export {bar};

```