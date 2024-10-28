# Reason
1. could be done in minifier
# Diff
## /a.js
### esbuild
```js
function foo(){let u;return(n=>(n[n.A=0]="A",n[n.B=1]="B",n[n.C=n]="C"))(u||={}),u}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/a.js
+++ rolldown	
@@ -1,4 +0,0 @@
-function foo() {
-    let u;
-    return ((n => (n[n.A = 0] = "A", n[n.B = 1] = "B", n[n.C = n] = "C"))(u ||= {}), u);
-}

```
## /b.js
### esbuild
```js
export function foo(){let e;return(n=>(n[n.X=0]="X",n[n.Y=1]="Y",n[n.Z=n]="Z"))(e||={}),e}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/b.js
+++ rolldown	
@@ -1,4 +0,0 @@
-export function foo() {
-    let e;
-    return ((n => (n[n.X = 0] = "X", n[n.Y = 1] = "Y", n[n.Z = n] = "Z"))(e ||= {}), e);
-}

```