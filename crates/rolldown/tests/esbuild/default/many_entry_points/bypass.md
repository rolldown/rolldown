# Reason
1. split chunks
# Diff
## /out/e00.js
### esbuild
```js
// shared.js
var shared_default = 123;

// e00.js
console.log(shared_default);
```
### rolldown
```js
import { shared_default } from "./shared.js";

//#region e00.js
console.log(shared_default);

```
### diff
```diff
===================================================================
--- esbuild	/out/e00.js
+++ rolldown	e00.js
@@ -1,2 +1,2 @@
-var shared_default = 123;
+import {shared_default} from "./shared.js";
 console.log(shared_default);

```
## /out/e01.js
### esbuild
```js
// shared.js
var shared_default = 123;

// e01.js
console.log(shared_default);
```
### rolldown
```js
import { shared_default } from "./shared.js";

//#region e01.js
console.log(shared_default);

```
### diff
```diff
===================================================================
--- esbuild	/out/e01.js
+++ rolldown	e01.js
@@ -1,2 +1,2 @@
-var shared_default = 123;
+import {shared_default} from "./shared.js";
 console.log(shared_default);

```
## /out/e02.js
### esbuild
```js
// shared.js
var shared_default = 123;

// e02.js
console.log(shared_default);
```
### rolldown
```js
import { shared_default } from "./shared.js";

//#region e02.js
console.log(shared_default);

```
### diff
```diff
===================================================================
--- esbuild	/out/e02.js
+++ rolldown	e02.js
@@ -1,2 +1,2 @@
-var shared_default = 123;
+import {shared_default} from "./shared.js";
 console.log(shared_default);

```
## /out/e03.js
### esbuild
```js
// shared.js
var shared_default = 123;

// e03.js
console.log(shared_default);
```
### rolldown
```js
import { shared_default } from "./shared.js";

//#region e03.js
console.log(shared_default);

```
### diff
```diff
===================================================================
--- esbuild	/out/e03.js
+++ rolldown	e03.js
@@ -1,2 +1,2 @@
-var shared_default = 123;
+import {shared_default} from "./shared.js";
 console.log(shared_default);

```
## /out/e04.js
### esbuild
```js
// shared.js
var shared_default = 123;

// e04.js
console.log(shared_default);
```
### rolldown
```js
import { shared_default } from "./shared.js";

//#region e04.js
console.log(shared_default);

```
### diff
```diff
===================================================================
--- esbuild	/out/e04.js
+++ rolldown	e04.js
@@ -1,2 +1,2 @@
-var shared_default = 123;
+import {shared_default} from "./shared.js";
 console.log(shared_default);

```
## /out/e05.js
### esbuild
```js
// shared.js
var shared_default = 123;

// e05.js
console.log(shared_default);
```
### rolldown
```js
import { shared_default } from "./shared.js";

//#region e05.js
console.log(shared_default);

```
### diff
```diff
===================================================================
--- esbuild	/out/e05.js
+++ rolldown	e05.js
@@ -1,2 +1,2 @@
-var shared_default = 123;
+import {shared_default} from "./shared.js";
 console.log(shared_default);

```
## /out/e06.js
### esbuild
```js
// shared.js
var shared_default = 123;

// e06.js
console.log(shared_default);
```
### rolldown
```js
import { shared_default } from "./shared.js";

//#region e06.js
console.log(shared_default);

```
### diff
```diff
===================================================================
--- esbuild	/out/e06.js
+++ rolldown	e06.js
@@ -1,2 +1,2 @@
-var shared_default = 123;
+import {shared_default} from "./shared.js";
 console.log(shared_default);

```
## /out/e07.js
### esbuild
```js
// shared.js
var shared_default = 123;

// e07.js
console.log(shared_default);
```
### rolldown
```js
import { shared_default } from "./shared.js";

//#region e07.js
console.log(shared_default);

```
### diff
```diff
===================================================================
--- esbuild	/out/e07.js
+++ rolldown	e07.js
@@ -1,2 +1,2 @@
-var shared_default = 123;
+import {shared_default} from "./shared.js";
 console.log(shared_default);

```
## /out/e08.js
### esbuild
```js
// shared.js
var shared_default = 123;

// e08.js
console.log(shared_default);
```
### rolldown
```js
import { shared_default } from "./shared.js";

//#region e08.js
console.log(shared_default);

```
### diff
```diff
===================================================================
--- esbuild	/out/e08.js
+++ rolldown	e08.js
@@ -1,2 +1,2 @@
-var shared_default = 123;
+import {shared_default} from "./shared.js";
 console.log(shared_default);

```
## /out/e09.js
### esbuild
```js
// shared.js
var shared_default = 123;

// e09.js
console.log(shared_default);
```
### rolldown
```js
import { shared_default } from "./shared.js";

//#region e09.js
console.log(shared_default);

```
### diff
```diff
===================================================================
--- esbuild	/out/e09.js
+++ rolldown	e09.js
@@ -1,2 +1,2 @@
-var shared_default = 123;
+import {shared_default} from "./shared.js";
 console.log(shared_default);

```
## /out/e10.js
### esbuild
```js
// shared.js
var shared_default = 123;

// e10.js
console.log(shared_default);
```
### rolldown
```js
import { shared_default } from "./shared.js";

//#region e10.js
console.log(shared_default);

```
### diff
```diff
===================================================================
--- esbuild	/out/e10.js
+++ rolldown	e10.js
@@ -1,2 +1,2 @@
-var shared_default = 123;
+import {shared_default} from "./shared.js";
 console.log(shared_default);

```
## /out/e11.js
### esbuild
```js
// shared.js
var shared_default = 123;

// e11.js
console.log(shared_default);
```
### rolldown
```js
import { shared_default } from "./shared.js";

//#region e11.js
console.log(shared_default);

```
### diff
```diff
===================================================================
--- esbuild	/out/e11.js
+++ rolldown	e11.js
@@ -1,2 +1,2 @@
-var shared_default = 123;
+import {shared_default} from "./shared.js";
 console.log(shared_default);

```
## /out/e12.js
### esbuild
```js
// shared.js
var shared_default = 123;

// e12.js
console.log(shared_default);
```
### rolldown
```js
import { shared_default } from "./shared.js";

//#region e12.js
console.log(shared_default);

```
### diff
```diff
===================================================================
--- esbuild	/out/e12.js
+++ rolldown	e12.js
@@ -1,2 +1,2 @@
-var shared_default = 123;
+import {shared_default} from "./shared.js";
 console.log(shared_default);

```
## /out/e13.js
### esbuild
```js
// shared.js
var shared_default = 123;

// e13.js
console.log(shared_default);
```
### rolldown
```js
import { shared_default } from "./shared.js";

//#region e13.js
console.log(shared_default);

```
### diff
```diff
===================================================================
--- esbuild	/out/e13.js
+++ rolldown	e13.js
@@ -1,2 +1,2 @@
-var shared_default = 123;
+import {shared_default} from "./shared.js";
 console.log(shared_default);

```
## /out/e14.js
### esbuild
```js
// shared.js
var shared_default = 123;

// e14.js
console.log(shared_default);
```
### rolldown
```js
import { shared_default } from "./shared.js";

//#region e14.js
console.log(shared_default);

```
### diff
```diff
===================================================================
--- esbuild	/out/e14.js
+++ rolldown	e14.js
@@ -1,2 +1,2 @@
-var shared_default = 123;
+import {shared_default} from "./shared.js";
 console.log(shared_default);

```
## /out/e15.js
### esbuild
```js
// shared.js
var shared_default = 123;

// e15.js
console.log(shared_default);
```
### rolldown
```js
import { shared_default } from "./shared.js";

//#region e15.js
console.log(shared_default);

```
### diff
```diff
===================================================================
--- esbuild	/out/e15.js
+++ rolldown	e15.js
@@ -1,2 +1,2 @@
-var shared_default = 123;
+import {shared_default} from "./shared.js";
 console.log(shared_default);

```
## /out/e16.js
### esbuild
```js
// shared.js
var shared_default = 123;

// e16.js
console.log(shared_default);
```
### rolldown
```js
import { shared_default } from "./shared.js";

//#region e16.js
console.log(shared_default);

```
### diff
```diff
===================================================================
--- esbuild	/out/e16.js
+++ rolldown	e16.js
@@ -1,2 +1,2 @@
-var shared_default = 123;
+import {shared_default} from "./shared.js";
 console.log(shared_default);

```
## /out/e17.js
### esbuild
```js
// shared.js
var shared_default = 123;

// e17.js
console.log(shared_default);
```
### rolldown
```js
import { shared_default } from "./shared.js";

//#region e17.js
console.log(shared_default);

```
### diff
```diff
===================================================================
--- esbuild	/out/e17.js
+++ rolldown	e17.js
@@ -1,2 +1,2 @@
-var shared_default = 123;
+import {shared_default} from "./shared.js";
 console.log(shared_default);

```
## /out/e18.js
### esbuild
```js
// shared.js
var shared_default = 123;

// e18.js
console.log(shared_default);
```
### rolldown
```js
import { shared_default } from "./shared.js";

//#region e18.js
console.log(shared_default);

```
### diff
```diff
===================================================================
--- esbuild	/out/e18.js
+++ rolldown	e18.js
@@ -1,2 +1,2 @@
-var shared_default = 123;
+import {shared_default} from "./shared.js";
 console.log(shared_default);

```
## /out/e19.js
### esbuild
```js
// shared.js
var shared_default = 123;

// e19.js
console.log(shared_default);
```
### rolldown
```js
import { shared_default } from "./shared.js";

//#region e19.js
console.log(shared_default);

```
### diff
```diff
===================================================================
--- esbuild	/out/e19.js
+++ rolldown	e19.js
@@ -1,2 +1,2 @@
-var shared_default = 123;
+import {shared_default} from "./shared.js";
 console.log(shared_default);

```
## /out/e20.js
### esbuild
```js
// shared.js
var shared_default = 123;

// e20.js
console.log(shared_default);
```
### rolldown
```js
import { shared_default } from "./shared.js";

//#region e20.js
console.log(shared_default);

```
### diff
```diff
===================================================================
--- esbuild	/out/e20.js
+++ rolldown	e20.js
@@ -1,2 +1,2 @@
-var shared_default = 123;
+import {shared_default} from "./shared.js";
 console.log(shared_default);

```
## /out/e21.js
### esbuild
```js
// shared.js
var shared_default = 123;

// e21.js
console.log(shared_default);
```
### rolldown
```js
import { shared_default } from "./shared.js";

//#region e21.js
console.log(shared_default);

```
### diff
```diff
===================================================================
--- esbuild	/out/e21.js
+++ rolldown	e21.js
@@ -1,2 +1,2 @@
-var shared_default = 123;
+import {shared_default} from "./shared.js";
 console.log(shared_default);

```
## /out/e22.js
### esbuild
```js
// shared.js
var shared_default = 123;

// e22.js
console.log(shared_default);
```
### rolldown
```js
import { shared_default } from "./shared.js";

//#region e22.js
console.log(shared_default);

```
### diff
```diff
===================================================================
--- esbuild	/out/e22.js
+++ rolldown	e22.js
@@ -1,2 +1,2 @@
-var shared_default = 123;
+import {shared_default} from "./shared.js";
 console.log(shared_default);

```
## /out/e23.js
### esbuild
```js
// shared.js
var shared_default = 123;

// e23.js
console.log(shared_default);
```
### rolldown
```js
import { shared_default } from "./shared.js";

//#region e23.js
console.log(shared_default);

```
### diff
```diff
===================================================================
--- esbuild	/out/e23.js
+++ rolldown	e23.js
@@ -1,2 +1,2 @@
-var shared_default = 123;
+import {shared_default} from "./shared.js";
 console.log(shared_default);

```
## /out/e24.js
### esbuild
```js
// shared.js
var shared_default = 123;

// e24.js
console.log(shared_default);
```
### rolldown
```js
import { shared_default } from "./shared.js";

//#region e24.js
console.log(shared_default);

```
### diff
```diff
===================================================================
--- esbuild	/out/e24.js
+++ rolldown	e24.js
@@ -1,2 +1,2 @@
-var shared_default = 123;
+import {shared_default} from "./shared.js";
 console.log(shared_default);

```
## /out/e25.js
### esbuild
```js
// shared.js
var shared_default = 123;

// e25.js
console.log(shared_default);
```
### rolldown
```js
import { shared_default } from "./shared.js";

//#region e25.js
console.log(shared_default);

```
### diff
```diff
===================================================================
--- esbuild	/out/e25.js
+++ rolldown	e25.js
@@ -1,2 +1,2 @@
-var shared_default = 123;
+import {shared_default} from "./shared.js";
 console.log(shared_default);

```
## /out/e26.js
### esbuild
```js
// shared.js
var shared_default = 123;

// e26.js
console.log(shared_default);
```
### rolldown
```js
import { shared_default } from "./shared.js";

//#region e26.js
console.log(shared_default);

```
### diff
```diff
===================================================================
--- esbuild	/out/e26.js
+++ rolldown	e26.js
@@ -1,2 +1,2 @@
-var shared_default = 123;
+import {shared_default} from "./shared.js";
 console.log(shared_default);

```
## /out/e27.js
### esbuild
```js
// shared.js
var shared_default = 123;

// e27.js
console.log(shared_default);
```
### rolldown
```js
import { shared_default } from "./shared.js";

//#region e27.js
console.log(shared_default);

```
### diff
```diff
===================================================================
--- esbuild	/out/e27.js
+++ rolldown	e27.js
@@ -1,2 +1,2 @@
-var shared_default = 123;
+import {shared_default} from "./shared.js";
 console.log(shared_default);

```
## /out/e28.js
### esbuild
```js
// shared.js
var shared_default = 123;

// e28.js
console.log(shared_default);
```
### rolldown
```js
import { shared_default } from "./shared.js";

//#region e28.js
console.log(shared_default);

```
### diff
```diff
===================================================================
--- esbuild	/out/e28.js
+++ rolldown	e28.js
@@ -1,2 +1,2 @@
-var shared_default = 123;
+import {shared_default} from "./shared.js";
 console.log(shared_default);

```
## /out/e29.js
### esbuild
```js
// shared.js
var shared_default = 123;

// e29.js
console.log(shared_default);
```
### rolldown
```js
import { shared_default } from "./shared.js";

//#region e29.js
console.log(shared_default);

```
### diff
```diff
===================================================================
--- esbuild	/out/e29.js
+++ rolldown	e29.js
@@ -1,2 +1,2 @@
-var shared_default = 123;
+import {shared_default} from "./shared.js";
 console.log(shared_default);

```
## /out/e30.js
### esbuild
```js
// shared.js
var shared_default = 123;

// e30.js
console.log(shared_default);
```
### rolldown
```js
import { shared_default } from "./shared.js";

//#region e30.js
console.log(shared_default);

```
### diff
```diff
===================================================================
--- esbuild	/out/e30.js
+++ rolldown	e30.js
@@ -1,2 +1,2 @@
-var shared_default = 123;
+import {shared_default} from "./shared.js";
 console.log(shared_default);

```
## /out/e31.js
### esbuild
```js
// shared.js
var shared_default = 123;

// e31.js
console.log(shared_default);
```
### rolldown
```js
import { shared_default } from "./shared.js";

//#region e31.js
console.log(shared_default);

```
### diff
```diff
===================================================================
--- esbuild	/out/e31.js
+++ rolldown	e31.js
@@ -1,2 +1,2 @@
-var shared_default = 123;
+import {shared_default} from "./shared.js";
 console.log(shared_default);

```
## /out/e32.js
### esbuild
```js
// shared.js
var shared_default = 123;

// e32.js
console.log(shared_default);
```
### rolldown
```js
import { shared_default } from "./shared.js";

//#region e32.js
console.log(shared_default);

```
### diff
```diff
===================================================================
--- esbuild	/out/e32.js
+++ rolldown	e32.js
@@ -1,2 +1,2 @@
-var shared_default = 123;
+import {shared_default} from "./shared.js";
 console.log(shared_default);

```
## /out/e33.js
### esbuild
```js
// shared.js
var shared_default = 123;

// e33.js
console.log(shared_default);
```
### rolldown
```js
import { shared_default } from "./shared.js";

//#region e33.js
console.log(shared_default);

```
### diff
```diff
===================================================================
--- esbuild	/out/e33.js
+++ rolldown	e33.js
@@ -1,2 +1,2 @@
-var shared_default = 123;
+import {shared_default} from "./shared.js";
 console.log(shared_default);

```
## /out/e34.js
### esbuild
```js
// shared.js
var shared_default = 123;

// e34.js
console.log(shared_default);
```
### rolldown
```js
import { shared_default } from "./shared.js";

//#region e34.js
console.log(shared_default);

```
### diff
```diff
===================================================================
--- esbuild	/out/e34.js
+++ rolldown	e34.js
@@ -1,2 +1,2 @@
-var shared_default = 123;
+import {shared_default} from "./shared.js";
 console.log(shared_default);

```
## /out/e35.js
### esbuild
```js
// shared.js
var shared_default = 123;

// e35.js
console.log(shared_default);
```
### rolldown
```js
import { shared_default } from "./shared.js";

//#region e35.js
console.log(shared_default);

```
### diff
```diff
===================================================================
--- esbuild	/out/e35.js
+++ rolldown	e35.js
@@ -1,2 +1,2 @@
-var shared_default = 123;
+import {shared_default} from "./shared.js";
 console.log(shared_default);

```
## /out/e36.js
### esbuild
```js
// shared.js
var shared_default = 123;

// e36.js
console.log(shared_default);
```
### rolldown
```js
import { shared_default } from "./shared.js";

//#region e36.js
console.log(shared_default);

```
### diff
```diff
===================================================================
--- esbuild	/out/e36.js
+++ rolldown	e36.js
@@ -1,2 +1,2 @@
-var shared_default = 123;
+import {shared_default} from "./shared.js";
 console.log(shared_default);

```
## /out/e37.js
### esbuild
```js
// shared.js
var shared_default = 123;

// e37.js
console.log(shared_default);
```
### rolldown
```js
import { shared_default } from "./shared.js";

//#region e37.js
console.log(shared_default);

```
### diff
```diff
===================================================================
--- esbuild	/out/e37.js
+++ rolldown	e37.js
@@ -1,2 +1,2 @@
-var shared_default = 123;
+import {shared_default} from "./shared.js";
 console.log(shared_default);

```
## /out/e38.js
### esbuild
```js
// shared.js
var shared_default = 123;

// e38.js
console.log(shared_default);
```
### rolldown
```js
import { shared_default } from "./shared.js";

//#region e38.js
console.log(shared_default);

```
### diff
```diff
===================================================================
--- esbuild	/out/e38.js
+++ rolldown	e38.js
@@ -1,2 +1,2 @@
-var shared_default = 123;
+import {shared_default} from "./shared.js";
 console.log(shared_default);

```
## /out/e39.js
### esbuild
```js
// shared.js
var shared_default = 123;

// e39.js
console.log(shared_default);
```
### rolldown
```js
import { shared_default } from "./shared.js";

//#region e39.js
console.log(shared_default);

```
### diff
```diff
===================================================================
--- esbuild	/out/e39.js
+++ rolldown	e39.js
@@ -1,2 +1,2 @@
-var shared_default = 123;
+import {shared_default} from "./shared.js";
 console.log(shared_default);

```