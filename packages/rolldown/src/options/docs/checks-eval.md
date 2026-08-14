##### What triggers this warning

```js
// main.js
const code = 'console.log("Hello")';
eval(code); // This triggers the warning
```

Direct `eval` poses security risks and prevents minification optimizations. Consider using indirect eval or alternative approaches:

```js
// Option 1: Indirect eval (evaluates in global scope)
const code = 'console.log("Hello")';
(0, eval)(code);

// Option 2: Use Function constructor
const fn = new Function('console.log("Hello")');
fn();
```
