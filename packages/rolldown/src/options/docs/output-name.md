Namespaces are supported i.e., your name can contain dots. The resulting bundle will contain the setup necessary for the namespacing.

```js
// output for `name: 'a.b.c'`
this.a = this.a || {};
this.a.b = this.a.b || {};
this.a.b.c = (function () {
  // ...
})();
```
