# Reason
1. different chunk naming style
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
import { bar } from "./foo.js";

//#region entry.js
import("./foo2.js").then(({ bar: b }) => console.log(bar, b));

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
+import {bar} from "./foo.js";
+import("./foo2.js").then(({bar: b}) => console.log(bar, b));

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

//#region foo.js
let bar = 123;

//#endregion
export { bar };
```
### diff
```diff
===================================================================
--- esbuild	/out/foo-BNHN4WV6.js
+++ rolldown	foo.js
@@ -1,2 +1,2 @@
-import {bar} from "./chunk-UDDKLWVZ.js";
+var bar = 123;
 export {bar};

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
import { bar } from "./foo.js";

export { bar };
```
### diff
```diff
===================================================================
--- esbuild	/out/chunk-UDDKLWVZ.js
+++ rolldown	foo2.js
@@ -1,2 +1,2 @@
-var bar = 123;
+import {bar} from "./foo.js";
 export {bar};

```