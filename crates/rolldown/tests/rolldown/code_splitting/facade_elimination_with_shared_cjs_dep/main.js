import assert from 'node:assert';

// This mimics the way import.meta.glob works in Vite.
const loaders = {
  ar: () => import('./vendor/antd/locale/ar_EG.js'),
  'en-US': () => import('./vendor/antd/locale/en_US.js'),
};

// A second dynamic entry in each group keeps the group chunks shared hosts.
const extraLoaders = {
  ar: () => import('./vendor/antd/locale/ar_EG_extra.js'),
  'en-US': () => import('./vendor/antd/locale/en_US_extra.js'),
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

  assert.strictEqual((await extraLoaders.ar()).extra, 'ar');
  assert.strictEqual((await extraLoaders['en-US']()).extra, 'en');
}

main();
