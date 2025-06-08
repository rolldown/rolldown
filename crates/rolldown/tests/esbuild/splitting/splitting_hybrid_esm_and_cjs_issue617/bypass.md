# Reason
1. different chunk naming style
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
import { b as foo, d as init_a } from "./a2.js";

init_a();
export { foo };
```
### diff
```diff
===================================================================
--- esbuild	/out/a.js
+++ rolldown	a.js
@@ -1,3 +1,3 @@
-import {foo, init_a} from "./chunk-PDZFCFBH.js";
+import {b as foo, d as init_a} from "./a2.js";
 init_a();
 export {foo};

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
import { c as a_exports, d as init_a, e as __toCommonJS } from "./a2.js";

//#region b.js
let bar = (init_a(), __toCommonJS(a_exports));

//#endregion
export { bar };
```
### diff
```diff
===================================================================
--- esbuild	/out/b.js
+++ rolldown	b.js
@@ -1,3 +1,3 @@
-import {__toCommonJS, a_exports, init_a} from "./chunk-PDZFCFBH.js";
+import {c as a_exports, d as init_a, e as __toCommonJS} from "./a2.js";
 var bar = (init_a(), __toCommonJS(a_exports));
 export {bar};

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

//#region a.js
var a_exports = {};
__export(a_exports, { foo: () => foo });
var foo;
var init_a = __esm({ "a.js"() {} });

//#endregion
export { foo as b, a_exports as c, init_a as d, __toCommonJS as e };
```
### diff
```diff
===================================================================
--- esbuild	/out/chunk-PDZFCFBH.js
+++ rolldown	a2.js
@@ -1,17 +1,9 @@
-// a.js
+
+//#region a.js
 var a_exports = {};
-__export(a_exports, {
-  foo: () => foo
-});
+__export(a_exports, { foo: () => foo });
 var foo;
-var init_a = __esm({
-  "a.js"() {
-  }
-});
+var init_a = __esm({ "a.js"() {} });
 
-export {
-  __toCommonJS,
-  foo,
-  a_exports,
-  init_a
-};
\ No newline at end of file
+//#endregion
+export { foo as b, a_exports as c, init_a as d, __toCommonJS as e };
\ No newline at end of file

```