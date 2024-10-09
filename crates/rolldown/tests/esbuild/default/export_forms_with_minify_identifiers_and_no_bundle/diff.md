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

```
### diff
```diff
===================================================================
--- esbuild	/out/a.js
+++ rolldown	
@@ -1,11 +0,0 @@
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

```
## /out/b.js
### esbuild
```js
export default function() {
}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/b.js
+++ rolldown	
@@ -1,1 +0,0 @@
-export default function () {}

```
## /out/c.js
### esbuild
```js
export default function o() {
}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/c.js
+++ rolldown	
@@ -1,1 +0,0 @@
-export default function o() {}

```
## /out/d.js
### esbuild
```js
export default class {
}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/d.js
+++ rolldown	
@@ -1,1 +0,0 @@
-export default class {}

```
## /out/e.js
### esbuild
```js
export default class o {
}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/e.js
+++ rolldown	
@@ -1,1 +0,0 @@
-export default class o {}

```