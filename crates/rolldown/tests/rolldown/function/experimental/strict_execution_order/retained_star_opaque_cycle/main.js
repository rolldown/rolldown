await import('./page-b.js');

const pageA = await import('./page-a.js');
const namespaceCopy = { ...pageA };

if (
  namespaceCopy.x() !== 1 ||
  namespaceCopy.common !== 'common' ||
  namespaceCopy.commonLeaf !== 'common' ||
  namespaceCopy._ !== undefined ||
  namespaceCopy.underscoreLeaf !== undefined
) {
  throw new Error('opaque dynamic import observed the wrong namespace');
}
