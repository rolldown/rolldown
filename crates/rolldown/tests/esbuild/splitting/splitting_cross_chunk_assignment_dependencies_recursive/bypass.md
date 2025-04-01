# Reason
1. different chunk naming style
# Diff
## /out/a.js
### esbuild
```js
import {
  setX
} from "./chunk-NAKBUG5G.js";

// a.js
setX();
```
### rolldown
```js
import { setX } from "./x.js";

//#region a.js
setX();
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/a.js
+++ rolldown	a.js
@@ -1,2 +1,2 @@
-import {setX} from "./chunk-NAKBUG5G.js";
+import {setX} from "./x.js";
 setX();

```
## /out/b.js
### esbuild
```js
import {
  setZ
} from "./chunk-BSMDVSN6.js";
import "./chunk-NAKBUG5G.js";

// b.js
setZ();
```
### rolldown
```js
import "./x.js";
import { setZ } from "./z.js";

//#region b.js
setZ();
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/b.js
+++ rolldown	b.js
@@ -1,3 +1,3 @@
-import {setZ} from "./chunk-BSMDVSN6.js";
-import "./chunk-NAKBUG5G.js";
+import "./x.js";
+import {setZ} from "./z.js";
 setZ();

```
## /out/c.js
### esbuild
```js
import {
  setY2,
  setZ2
} from "./chunk-BSMDVSN6.js";
import {
  setX2
} from "./chunk-NAKBUG5G.js";

// c.js
setX2();
setY2();
setZ2();
```
### rolldown
```js
import { setX2 } from "./x.js";
import { setY2, setZ2 } from "./z.js";

//#region c.js
setX2();
setY2();
setZ2();
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/c.js
+++ rolldown	c.js
@@ -1,5 +1,5 @@
-import {setY2, setZ2} from "./chunk-BSMDVSN6.js";
-import {setX2} from "./chunk-NAKBUG5G.js";
+import {setX2} from "./x.js";
+import {setY2, setZ2} from "./z.js";
 setX2();
 setY2();
 setZ2();

```
## /out/chunk-BSMDVSN6.js
### esbuild
```js
import {
  setX
} from "./chunk-NAKBUG5G.js";

// y.js
var _y;
function setY(v) {
  _y = v;
}
function setY2(v) {
  setX(v);
  _y = v;
}

// z.js
var _z;
function setZ(v) {
  _z = v;
}
function setZ2(v) {
  setY(v);
  _z = v;
}

export {
  setY2,
  setZ,
  setZ2
};
```
### rolldown
```js
import { setX } from "./x.js";

//#region y.js
let _y;
function setY(v) {
	_y = v;
}
function setY2(v) {
	setX(v);
	_y = v;
}
//#endregion

//#region z.js
let _z;
function setZ(v) {
	_z = v;
}
function setZ2(v) {
	setY(v);
	_z = v;
}
//#endregion

export { setY2, setZ, setZ2 };
```
### diff
```diff
===================================================================
--- esbuild	/out/chunk-BSMDVSN6.js
+++ rolldown	z.js
@@ -1,5 +1,5 @@
-import {setX} from "./chunk-NAKBUG5G.js";
+import {setX} from "./x.js";
 var _y;
 function setY(v) {
     _y = v;
 }

```