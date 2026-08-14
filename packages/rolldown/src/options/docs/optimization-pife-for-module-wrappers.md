::: tip What is PIFE?

PIFE is the abbreviation of "Possibly-Invoked Function Expressions". It is a function expression wrapped with a parenthesized expression.

PIFEs annotate functions that are likely to be invoked eagerly. When [V8 JavaScript engine](https://v8.dev/) (the engine used in Chrome and Node.js) encounters such expressions, it compiles them eagerly (rather than compiling it later). See [V8's blog post](https://v8.dev/blog/preparser#pife) for more details.

:::
