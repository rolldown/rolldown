# Diff
## /out.js
### esbuild
```js
var t=e(r=>{r.foo=function(){return 123}});var n=e((l,c)=>{c.exports={test:!0}});var{foo:f}=t();console.log(f(),n());
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	
@@ -1,12 +0,0 @@
-var t = e(r => {
-    r.foo = function () {
-        return 123;
-    };
-});
-var n = e((l, c) => {
-    c.exports = {
-        test: !0
-    };
-});
-var {foo: f} = t();
-console.log(f(), n());

```