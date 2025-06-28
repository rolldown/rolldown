import assert from 'node:assert';
// import * as ns from './src/basic_ns/'
// import default_cjs from './src/basic_ref_with_named_default'

import * as export_star_from_cjs from './src/export_star_from_cjs/'

// assert.equal(ns.a, 'basic-a')
//
// assert.equal(default_cjs.a, 'basic_ref_with_named_default_a')

assert.equal(export_star_from_cjs.a, 'export_star_from_cjs_a')


