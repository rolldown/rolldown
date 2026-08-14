import assert from 'node:assert';

// This mimics the way import.meta.glob works in Vite.
const loaders = {
  ar: () => import('./vendor/antd/locale/ar_EG.js'),
  'en-US': () => import('./vendor/antd/locale/en_US.js'),
};

async function main() {
  const ar = await loaders.ar();
  const enUS = await loaders['en-US']();

  assert.strictEqual(ar.locale, 'ar');
  assert.strictEqual(ar.DatePicker.locale, 'ar');
  assert.strictEqual(ar.DatePicker.yearFormat, 'YYYY');

  assert.strictEqual(enUS.locale, 'en');
  assert.strictEqual(enUS.DatePicker.locale, 'en_US');
  assert.strictEqual(enUS.DatePicker.yearFormat, 'YYYY');
}

main();
