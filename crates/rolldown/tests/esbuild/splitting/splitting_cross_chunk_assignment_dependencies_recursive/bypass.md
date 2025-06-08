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
import { c as setX } from "./x.js";

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
+import {c as setX} from "./x.js";
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
import { b as setZ } from "./z.js";

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
+import {b as setZ} from "./z.js";
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
import { b as setX2 } from "./x.js";
import { c as setZ2, d as setY2 } from "./z.js";

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
+import {b as setX2} from "./x.js";
+import {c as setZ2, d as setY2} from "./z.js";
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
import { c as setX } from "./x.js";

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
export { setZ as b, setZ2 as c, setY2 as d };
```
### diff
```diff
===================================================================
--- esbuild	/out/chunk-BSMDVSN6.js
+++ rolldown	z.js
@@ -1,5 +1,5 @@
-import {setX} from "./chunk-NAKBUG5G.js";
+import {c as setX} from "./x.js";
 var _y;
 function setY(v) {
     _y = v;
 }
@@ -14,5 +14,5 @@
 function setZ2(v) {
     setY(v);
     _z = v;
 }
-export {setY2, setZ, setZ2};
+export {setZ as b, setZ2 as c, setY2 as d};

```
## /out/chunk-NAKBUG5G.js
### esbuild
```js
// x.js
var _x;
function setX(v) {
  _x = v;
}
function setX2(v) {
  _x = v;
}

export {
  setX,
  setX2
};
```
### rolldown
```js
//#region x.js
let _x;
function setX(v) {
	_x = v;
}
function setX2(v) {
	_x = v;
}

//#endregion
export { setX2 as b, setX as c };
```
### diff
```diff
===================================================================
--- esbuild	/out/chunk-NAKBUG5G.js
+++ rolldown	x.js
@@ -4,5 +4,5 @@
 }
 function setX2(v) {
     _x = v;
 }
-export {setX, setX2};
+export {setX2 as b, setX as c};

```