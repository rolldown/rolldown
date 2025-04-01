# Reason
1. should not generate duplicate export binding
# Diff
## /out/a.js
### esbuild
```js
export default 123;
export var varName = 234;
export let letName = 234;
export const constName = 234;
function s() {
}
class t {
}
export { Class as Cls, s as Fn2, t as Cls2 };
export function Func() {
}
export class Class {
}
export * from "./a";
export * as fromB from "./b";
```
### rolldown
```js
import { b_exports } from "./b2.js";

//#region a.js
var a_default = 123;
var varName = 234;
let letName = 234;
const constName = 234;
function Func2() {}
var Class2 = class {};
function Func() {}
var Class = class {};
//#endregion

export { Class, Class as Cls, Class2 as Cls2, Func2 as Fn2, Func, constName, a_default as default, b_exports as fromB, letName, varName };
```
### diff
```diff
===================================================================
--- esbuild	/out/a.js
+++ rolldown	a.js
@@ -1,11 +1,10 @@
-export default 123;
-export var varName = 234;
-export let letName = 234;
-export const constName = 234;
-function s() {}
-class t {}
-export {Class as Cls, s as Fn2, t as Cls2};
-export function Func() {}
-export class Class {}
-export * from "./a";
-export * as fromB from "./b";
+import {b_exports} from "./b2.js";
+var a_default = 123;
+var varName = 234;
+var letName = 234;
+var constName = 234;
+function Func2() {}
+var Class2 = class {};
+function Func() {}
+var Class = class {};
+export {Class, Class as Cls, Class2 as Cls2, Func2 as Fn2, Func, constName, a_default as default, b_exports as fromB, letName, varName};

```
## /out/b.js
### esbuild
```js
export default function() {
}
```
### rolldown
```js
import { b_default } from "./b2.js";

export { b_default as default };
```
### diff
```diff
===================================================================
--- esbuild	/out/b.js
+++ rolldown	b.js
@@ -1,1 +1,2 @@
-export default function () {}
+import {b_default} from "./b2.js";
+export {b_default as default};

```
## /out/c.js
### esbuild
```js
export default function o() {
}
```
### rolldown
```js

//#region c.js
function foo() {}
//#endregion

export { foo as default };
```
### diff
```diff
===================================================================
--- esbuild	/out/c.js
+++ rolldown	c.js
@@ -1,1 +1,2 @@
-export default function o() {}
+function foo() {}
+export {foo as default};

```
## /out/d.js
### esbuild
```js
export default class {
}
```
### rolldown
```js

//#region d.js
var d_default = class {};
//#endregion

export { d_default as default };
```
### diff
```diff
===================================================================
--- esbuild	/out/d.js
+++ rolldown	d.js
@@ -1,1 +1,2 @@
-export default class {}
+var d_default = class {};
+export {d_default as default};

```
## /out/e.js
### esbuild
```js
export default class o {
}
```
### rolldown
```js

//#region e.js
var Foo = class {};
//#endregion

export { Foo as default };
```
### diff
```diff
===================================================================
--- esbuild	/out/e.js
+++ rolldown	e.js
@@ -1,1 +1,2 @@
-export default class o {}
+var Foo = class {};
+export {Foo as default};

```