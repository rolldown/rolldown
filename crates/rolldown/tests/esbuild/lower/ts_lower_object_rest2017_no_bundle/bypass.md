# Reason
1. pure transformation is handled by `oxc-transform`
# Diff
## /out.js
### esbuild
```js
var _q, _r, _t, _u, _v, _w, _x;
const local_const = __objRest({}, []);
let local_let = __objRest({}, []);
var local_var = __objRest({}, []);
let arrow_fn = (_a) => {
  var x2 = __objRest(_a, []);
};
let fn_expr = function(_b = default_value) {
  var x2 = __objRest(_b, []);
};
let class_expr = class {
  method(x2, ..._c) {
    var [y, _d] = _c, z = __objRest(_d, []);
  }
};
function fn_stmt(_e, _g) {
  var _f = _e, { a = b() } = _f, x2 = __objRest(_f, ["a"]);
  var _h = _g, { c = d() } = _h, y = __objRest(_h, ["c"]);
}
class class_stmt {
  method(_i) {
    var x2 = __objRest(_i, []);
  }
}
var ns;
((ns2) => {
  ns2.x = __objRest({}, []);
})(ns || (ns = {}));
try {
} catch (_j) {
  let catch_clause = __objRest(_j, []);
}
for (const _k in { abc }) {
  const for_in_const = __objRest(_k, []);
}
for (let _l in { abc }) {
  let for_in_let = __objRest(_l, []);
}
for (var _m in { abc }) {
  var for_in_var = __objRest(_m, []);
  ;
}
for (const _n of [{}]) {
  const for_of_const = __objRest(_n, []);
  ;
}
for (let _o of [{}]) {
  let for_of_let = __objRest(_o, []);
  x();
}
for (var _p of [{}]) {
  var for_of_var = __objRest(_p, []);
  x();
}
for (const for_const = __objRest({}, []); x; x = null) {
}
for (let for_let = __objRest({}, []); x; x = null) {
}
for (var for_var = __objRest({}, []); x; x = null) {
}
for (_q in { abc }) {
  x = __objRest(_q, []);
}
for (_r of [{}]) {
  x = __objRest(_r, []);
}
for (x = __objRest({}, []); x; x = null) {
}
assign = __objRest({}, []);
({ obj_method(_s) {
  var x2 = __objRest(_s, []);
} });
x = __objRest(x, []);
for (x = __objRest(x, []); 0; ) ;
console.log((x = __objRest(_t = x, []), _t));
console.log((_v = _u = { x }, { x } = _v, xx = __objRest(_v, ["x"]), _u));
console.log(({ x: _x } = _w = { x }, xx = __objRest(_x, []), _w));
```
### rolldown
```js

//#region entry.ts
const { ...local_const } = {};
let { ...local_let } = {};
var { ...local_var } = {};
let ns;
(function(_ns) {
	let { ...x$1 } = {};
	_ns.x = x$1;
})(ns || (ns = {}));
for (const { ...for_in_const } in { abc });
for (let { ...for_in_let } in { abc });
for (var { ...for_in_var } in { abc });
for (const { ...for_of_const } of [{}]);
for (let { ...for_of_let } of [{}]) x();
for (var { ...for_of_var } of [{}]) x();
for (const { ...for_const } = {}; x; x = null);
for (let { ...for_let } = {}; x; x = null);
for (var { ...for_var } = {}; x; x = null);
for ({...x} in { abc });
for ({...x} of [{}]);
for ({...x} = {}; x; x = null);
({...assign} = {});
({...x} = x);
for ({...x} = x; 0;);
console.log({...x} = x);
console.log({x,...xx} = { x });
console.log({x: {...xx}} = { x });
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,86 +1,38 @@
-var _q, _r, _t, _u, _v, _w, _x;
-const local_const = __objRest({}, []);
-let local_let = __objRest({}, []);
-var local_var = __objRest({}, []);
-let arrow_fn = _a => {
-    var x2 = __objRest(_a, []);
-};
-let fn_expr = function (_b = default_value) {
-    var x2 = __objRest(_b, []);
-};
-let class_expr = class {
-    method(x2, ..._c) {
-        var [y, _d] = _c, z = __objRest(_d, []);
-    }
-};
-function fn_stmt(_e, _g) {
-    var _f = _e, {a = b()} = _f, x2 = __objRest(_f, ["a"]);
-    var _h = _g, {c = d()} = _h, y = __objRest(_h, ["c"]);
-}
-class class_stmt {
-    method(_i) {
-        var x2 = __objRest(_i, []);
-    }
-}
+var {...local_const} = {};
+var {...local_let} = {};
+var {...local_var} = {};
 var ns;
-(ns2 => {
-    ns2.x = __objRest({}, []);
+(function (_ns) {
+    let {...x$1} = {};
+    _ns.x = x$1;
 })(ns || (ns = {}));
-try {} catch (_j) {
-    let catch_clause = __objRest(_j, []);
-}
-for (const _k in {
+for (const {...for_in_const} in {
     abc
-}) {
-    const for_in_const = __objRest(_k, []);
-}
-for (let _l in {
+}) ;
+for (let {...for_in_let} in {
     abc
-}) {
-    let for_in_let = __objRest(_l, []);
-}
-for (var _m in {
+}) ;
+for (var {...for_in_var} in {
     abc
-}) {
-    var for_in_var = __objRest(_m, []);
-    ;
-}
-for (const _n of [{}]) {
-    const for_of_const = __objRest(_n, []);
-    ;
-}
-for (let _o of [{}]) {
-    let for_of_let = __objRest(_o, []);
-    x();
-}
-for (var _p of [{}]) {
-    var for_of_var = __objRest(_p, []);
-    x();
-}
-for (const for_const = __objRest({}, []); x; x = null) {}
-for (let for_let = __objRest({}, []); x; x = null) {}
-for (var for_var = __objRest({}, []); x; x = null) {}
-for (_q in {
+}) ;
+for (const {...for_of_const} of [{}]) ;
+for (let {...for_of_let} of [{}]) x();
+for (var {...for_of_var} of [{}]) x();
+for (const {...for_const} = {}; x; x = null) ;
+for (let {...for_let} = {}; x; x = null) ;
+for (var {...for_var} = {}; x; x = null) ;
+for ({...x} in {
     abc
-}) {
-    x = __objRest(_q, []);
-}
-for (_r of [{}]) {
-    x = __objRest(_r, []);
-}
-for (x = __objRest({}, []); x; x = null) {}
-assign = __objRest({}, []);
-({
-    obj_method(_s) {
-        var x2 = __objRest(_s, []);
-    }
+}) ;
+for ({...x} of [{}]) ;
+for ({...x} = {}; x; x = null) ;
+({...assign} = {});
+({...x} = x);
+for ({...x} = x; 0; ) ;
+console.log({...x} = x);
+console.log({x, ...xx} = {
+    x
 });
-x = __objRest(x, []);
-for (x = __objRest(x, []); 0; ) ;
-console.log((x = __objRest(_t = x, []), _t));
-console.log((_v = _u = {
+console.log({x: {...xx}} = {
     x
-}, {x} = _v, xx = __objRest(_v, ["x"]), _u));
-console.log(({x: _x} = _w = {
-    x
-}, xx = __objRest(_x, []), _w));
+});

```