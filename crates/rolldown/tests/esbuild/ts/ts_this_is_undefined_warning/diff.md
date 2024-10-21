# Reason
1. rewrite this when it is undefined
# Diff
## /out/warning1.js
### esbuild
```js
// warning1.ts
var foo = void 0;
export {
  foo
};
```
### rolldown
```js

//#region warning1.ts
var foo = this;

//#endregion
export { foo };
```
### diff
```diff
===================================================================
--- esbuild	/out/warning1.js
+++ rolldown	warning1.js
@@ -1,2 +1,2 @@
-var foo = void 0;
+var foo = this;
 export {foo};

```
## /out/warning2.js
### esbuild
```js
// warning2.ts
var foo = (void 0).foo;
export {
  foo
};
```
### rolldown
```js

//#region warning2.ts
var foo = this || this.foo;

//#endregion
export { foo };
```
### diff
```diff
===================================================================
--- esbuild	/out/warning2.js
+++ rolldown	warning2.js
@@ -1,2 +1,2 @@
-var foo = (void 0).foo;
+var foo = this || this.foo;
 export {foo};

```
## /out/warning3.js
### esbuild
```js
// warning3.ts
var foo = void 0 ? (void 0).foo : null;
export {
  foo
};
```
### rolldown
```js

//#region warning3.ts
var foo = this ? this.foo : null;

//#endregion
export { foo };
```
### diff
```diff
===================================================================
--- esbuild	/out/warning3.js
+++ rolldown	warning3.js
@@ -1,2 +1,2 @@
-var foo = void 0 ? (void 0).foo : null;
+var foo = this ? this.foo : null;
 export {foo};

```
## /out/silent1.js
### esbuild
```js
// silent1.ts
var foo = void 0;
export {
  foo
};
```
### rolldown
```js

//#region silent1.ts
var foo = this && this.foo;

//#endregion
export { foo };
```
### diff
```diff
===================================================================
--- esbuild	/out/silent1.js
+++ rolldown	silent1.js
@@ -1,2 +1,2 @@
-var foo = void 0;
+var foo = this && this.foo;
 export {foo};

```
## /out/silent2.js
### esbuild
```js
// silent2.ts
var foo = void 0;
export {
  foo
};
```
### rolldown
```js

//#region silent2.ts
var foo = this && (() => this.foo);

//#endregion
export { foo };
```
### diff
```diff
===================================================================
--- esbuild	/out/silent2.js
+++ rolldown	silent2.js
@@ -1,2 +1,2 @@
-var foo = void 0;
+var foo = this && (() => this.foo);
 export {foo};

```