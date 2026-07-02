// Consumer imports ONLY HandlerA from the barrel. Because lib/index.js is an
// `export *` barrel, resolving `HandlerA` makes rolldown search every star
// source — including the dead handler-b.js. handler-b's own export (HandlerB) is
// never used.
import { HandlerA } from './lib/index.js';

// A dynamic import. With `codeSplitting: false` rolldown inlines it and WRAPS the
// imported module; the wrap propagates through the barrel's `export *` and
// force-includes handler-b.js's body (Problem 1). With lazyBarrel enabled the
// import that body needs is then dropped (Problem 2) -> ReferenceError.
import('./trigger.js');

console.log(HandlerA);
