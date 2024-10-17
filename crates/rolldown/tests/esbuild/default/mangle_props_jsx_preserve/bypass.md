# Reason
1. could be done in minifier
# Diff
## /out.jsx
### esbuild
```js
let Foo = {
  a(props) {
    return <>{props.b}</>;
  },
  c: "hello, world"
};
export default <Foo.a b={Foo.c} />;
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out.jsx
+++ rolldown	
@@ -1,7 +0,0 @@
-let Foo = {
-  a(props) {
-    return <>{props.b}</>;
-  },
-  c: "hello, world"
-};
-export default <Foo.a b={Foo.c} />;
\ No newline at end of file

```