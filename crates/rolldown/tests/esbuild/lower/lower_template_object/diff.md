# Diff
## /out.js
### esbuild
```js
var _a, _b, _c, _d, _e, _f, _g, _h;
x = () => [
  tag(_a || (_a = __template(["x"]))),
  tag(_b || (_b = __template(["ÿ"], ["\\xFF"]))),
  tag(_c || (_c = __template([void 0], ["\\x"]))),
  tag(_d || (_d = __template([void 0], ["\\u"])))
];
y = () => [
  tag(_e || (_e = __template(["x", "z"])), y),
  tag(_f || (_f = __template(["ÿ", "z"], ["\\xFF", "z"])), y),
  tag(_g || (_g = __template(["x", "z"], ["x", "\\z"])), y),
  tag(_h || (_h = __template(["x", void 0], ["x", "\\u"])), y)
];
```
### rolldown
```js


```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,3 +0,0 @@
-var _a, _b, _c, _d, _e, _f, _g, _h;
-x = () => [tag(_a || (_a = __template(["x"]))), tag(_b || (_b = __template(["ÿ"], ["\\xFF"]))), tag(_c || (_c = __template([void 0], ["\\x"]))), tag(_d || (_d = __template([void 0], ["\\u"])))];
-y = () => [tag(_e || (_e = __template(["x", "z"])), y), tag(_f || (_f = __template(["ÿ", "z"], ["\\xFF", "z"])), y), tag(_g || (_g = __template(["x", "z"], ["x", "\\z"])), y), tag(_h || (_h = __template(["x", void 0], ["x", "\\u"])), y)];

```