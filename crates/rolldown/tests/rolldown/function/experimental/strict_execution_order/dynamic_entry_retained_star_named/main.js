await import('./page-b.js');

const pageA = await import('./page-a.js');
console.log(pageA.common, pageA._);
