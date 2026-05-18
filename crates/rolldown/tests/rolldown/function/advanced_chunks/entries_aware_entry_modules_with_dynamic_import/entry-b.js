const module = await import('./shared.js');
module.foo();
console.log('side-effect-b');
