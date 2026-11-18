import type {
  BindingInjectImport,
  BindingInjectImportNamed,
  BindingInjectImportNamespace,
} from '../binding.cjs';

export function bindingifyInject(
  inject: Record<string, string | [string, string]> | undefined,
): BindingInjectImport[] | undefined {
  if (inject) {
    return Object.entries(inject).map(
      ([alias, item]): BindingInjectImportNamed | BindingInjectImportNamespace => {
        if (Array.isArray(item)) {
          // import * as fs from 'node:fs'
          // fs: ['node:fs', '*' ],
          if (item[1] === '*') {
            return {
              tagNamespace: true,
              alias,
              from: item[0],
            };
          }

          // import { Promise } from 'es6-promise'
          // Promise: [ 'es6-promise', 'Promise' ],

          // import { Promise as P } from 'es6-promise'
          // P: [ 'es6-promise', 'Promise' ],
          return {
            tagNamed: true,
            alias,
            from: item[0],
            imported: item[1],
          };
        } else {
          // import $ from 'jquery'
          // $: 'jquery',

          // 'Object.assign': path.resolve( 'src/helpers/object-assign.js' ),
          return {
            tagNamed: true,
            imported: 'default',
            alias,
            from: item,
          };
        }
      },
    );
  }
}
