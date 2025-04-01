# Reason
1. don't support dce iife
# Diff
## /out/remove-these.js
### esbuild
```js
keepThisButRemoveTheIIFE;
```
### rolldown
```js

//#region remove-these.js
(() => {})(keepThisButRemoveTheIIFE);
var someVar;
((x) => {})(someVar);
var removeThis2 = (() => 123)();
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/remove-these.js
+++ rolldown	remove-these.js
@@ -1,1 +1,4 @@
-keepThisButRemoveTheIIFE;
+(() => {})(keepThisButRemoveTheIIFE);
+var someVar;
+(x => {})(someVar);
+var removeThis2 = (() => 123)();

```
## /out/keep-these.js
### esbuild
```js
undef = void 0;
keepMe();
((x = keepMe()) => {
})();
var someVar;
(([y]) => {
})(someVar);
(({ z }) => {
})(someVar);
var keepThis = stuff();
keepThis();
((_ = keepMe()) => {
})();
var isPure = /* @__PURE__ */ ((x, y) => 123)();
use(isPure);
var isNotPure = ((x = foo, y = bar) => 123)();
use(isNotPure);
(async () => ({ get then() {
  notPure();
} }))();
(async function() {
  return { get then() {
    notPure();
  } };
})();
```
### rolldown
```js

//#region keep-these.js
undef = (() => {})();
keepMe();
((x = keepMe()) => {})();
var someVar;
(([y]) => {})(someVar);
(({ z }) => {})(someVar);
var keepThis = /* @__PURE__ */ (() => stuff())();
keepThis();
((_ = keepMe()) => {})();
var isPure = ((x, y) => 123)();
use(isPure);
var isNotPure = ((x = foo, y = bar) => 123)();
use(isNotPure);
(async () => ({ get then() {
	notPure();
} }))();
(async function() {
	return { get then() {
		notPure();
	} };
})();
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/keep-these.js
+++ rolldown	keep-these.js
@@ -1,11 +1,11 @@
-undef = void 0;
+undef = (() => {})();
 keepMe();
 ((x = keepMe()) => {})();
 var someVar;
 (([y]) => {})(someVar);
 (({z}) => {})(someVar);
-var keepThis = stuff();
+var keepThis = (() => stuff())();
 keepThis();
 ((_ = keepMe()) => {})();
 var isPure = ((x, y) => 123)();
 use(isPure);

```