await import('./page-b.js');

const pageA = await import('./page-a.js');
console.log(pageA.commonA, pageA.commonB);
