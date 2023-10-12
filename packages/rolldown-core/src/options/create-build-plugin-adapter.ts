// import type { Plugin } from '../rollup-types'
// import type { BuildPluginOption } from '@rolldown/node-binding'
// import { unimplemented } from '../utils'

// export function createBuildPluginAdapter(plugin: Plugin): BuildPluginOption {
//   // TODO: Need to investigate how to pass context to plugin.
//   const context: any = null
//   return {
//     name: plugin.name ?? 'unknown',
//     transform: async (code, id) => {
//       const transform = plugin.transform
//       if (transform == null) {
//         return null
//       }
//       const handler = (function () {
//         if (typeof transform === 'function') {
//           return transform
//         } else {
//           return transform.handler
//         }
//       })()

//       const ret = await handler.call(context, code, id)

//       if (typeof ret === 'string' || ret == null) {
//         return ret ?? null
//       }

//       if ('code' in ret) {
//         // TODO: we don't supports source map yet.
//         return ret.code
//       }
//     },
//     resolveId: !plugin.resolveId
//       ? undefined
//       : async (specifier, importer) => {
//           const resolveId = plugin.resolveId
//           if (resolveId == null) {
//             return null
//           }

//           const handler = (function () {
//             if (typeof resolveId === 'function') {
//               return resolveId
//             } else {
//               return resolveId.handler
//             }
//           })()

//           const ret = await handler.call(context, specifier, importer, {
//             assertions: {},
//             get isEntry() {
//               return unimplemented()
//             },
//           })
//           if (typeof ret === 'string') {
//             return {
//               id: ret,
//               external: false,
//             }
//           } else if (!ret || ret == null) {
//             return null
//           } else {
//             if (ret?.external === 'absolute' || ret?.external === 'relative') {
//               throw new Error(
//                 `External module type {${ret.external}} is not supported yet.`,
//               )
//             }
//             return {
//               id: ret.id,
//               external: ret.external ?? false,
//             }
//           }
//         },
//   }
// }
