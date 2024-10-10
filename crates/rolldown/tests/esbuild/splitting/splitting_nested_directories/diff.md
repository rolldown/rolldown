## /Users/user/project/out/pageA/page.js
### esbuild
```js
import {
  shared_default
} from "../chunk-GWC2ABNX.js";

// Users/user/project/src/pages/pageA/page.js
console.log(shared_default);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out/pageA/page.js
+++ rolldown	
@@ -1,2 +0,0 @@
-import {shared_default} from "../chunk-GWC2ABNX.js";
-console.log(shared_default);

```
## /Users/user/project/out/pageB/page.js
### esbuild
```js
import {
  shared_default
} from "../chunk-GWC2ABNX.js";

// Users/user/project/src/pages/pageB/page.js
console.log(-shared_default);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out/pageB/page.js
+++ rolldown	
@@ -1,2 +0,0 @@
-import {shared_default} from "../chunk-GWC2ABNX.js";
-console.log(-shared_default);

```
## /Users/user/project/out/chunk-GWC2ABNX.js
### esbuild
```js
// Users/user/project/src/pages/shared.js
var shared_default = 123;

export {
  shared_default
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out/chunk-GWC2ABNX.js
+++ rolldown	
@@ -1,2 +0,0 @@
-var shared_default = 123;
-export {shared_default};

```
# Diff
## /Users/user/project/out/pageA/page.js
### esbuild
```js
import {
  shared_default
} from "../chunk-GWC2ABNX.js";

// Users/user/project/src/pages/pageA/page.js
console.log(shared_default);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out/pageA/page.js
+++ rolldown	
@@ -1,2 +0,0 @@
-import {shared_default} from "../chunk-GWC2ABNX.js";
-console.log(shared_default);

```
## /Users/user/project/out/pageB/page.js
### esbuild
```js
import {
  shared_default
} from "../chunk-GWC2ABNX.js";

// Users/user/project/src/pages/pageB/page.js
console.log(-shared_default);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out/pageB/page.js
+++ rolldown	
@@ -1,2 +0,0 @@
-import {shared_default} from "../chunk-GWC2ABNX.js";
-console.log(-shared_default);

```
## /Users/user/project/out/chunk-GWC2ABNX.js
### esbuild
```js
// Users/user/project/src/pages/shared.js
var shared_default = 123;

export {
  shared_default
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out/chunk-GWC2ABNX.js
+++ rolldown	
@@ -1,2 +0,0 @@
-var shared_default = 123;
-export {shared_default};

```