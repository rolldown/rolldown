await import('./page-b.js');

const pageA = await import('./page-a.js');

if (pageA.y !== 1000 || pageA.common !== 'common' || pageA._ !== undefined) {
  throw new Error('partial dynamic import observed the wrong constant');
}
