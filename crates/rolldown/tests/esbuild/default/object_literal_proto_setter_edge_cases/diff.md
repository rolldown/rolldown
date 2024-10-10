# Diff
## /out/import-normal.js
### esbuild
```js
import { __proto__, bar } from "foo";
function foo() {
  console.log(
    'this must not become "{ __proto__ }":',
    {
      __proto__: __proto__,
      bar
    }
  );
}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/import-normal.js
+++ rolldown	
@@ -1,7 +0,0 @@
-import {__proto__, bar} from "foo";
-function foo() {
-    console.log('this must not become "{ __proto__ }":', {
-        __proto__: __proto__,
-        bar
-    });
-}

```
## /out/import-shorthand.js
### esbuild
```js
import { __proto__, bar } from "foo";
function foo() {
  console.log(
    'this must not become "{ __proto__: ... }":',
    {
      __proto__,
      bar
    }
  );
}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/import-shorthand.js
+++ rolldown	
@@ -1,7 +0,0 @@
-import {__proto__, bar} from "foo";
-function foo() {
-    console.log('this must not become "{ __proto__: ... }":', {
-        __proto__,
-        bar
-    });
-}

```
## /out/local-normal.js
### esbuild
```js
function foo(__proto__, bar) {
  console.log(
    'this must not become "{ __proto__ }":',
    {
      __proto__: __proto__,
      bar
    }
  );
}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/local-normal.js
+++ rolldown	
@@ -1,6 +0,0 @@
-function foo(__proto__, bar) {
-    console.log('this must not become "{ __proto__ }":', {
-        __proto__: __proto__,
-        bar
-    });
-}

```
## /out/local-shorthand.js
### esbuild
```js
function foo(__proto__, bar) {
  {
    let __proto__2, bar2;
    console.log(
      'this must not become "{ __proto__: ... }":',
      {
        ["__proto__"]: __proto__2,
        bar: bar2
      }
    );
  }
}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/local-shorthand.js
+++ rolldown	
@@ -1,9 +0,0 @@
-function foo(__proto__, bar) {
-    {
-        let __proto__2, bar2;
-        console.log('this must not become "{ __proto__: ... }":', {
-            ["__proto__"]: __proto__2,
-            bar: bar2
-        });
-    }
-}

```