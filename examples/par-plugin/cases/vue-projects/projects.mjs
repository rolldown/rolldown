import nodePath from 'node:path';

export const REPOSITORY_ROOT = nodePath.resolve(import.meta.dirname, '../../../..');
export const CORPUS_ROOT = nodePath.join(REPOSITORY_ROOT, 'tmp/bench/vue-projects');

export const REQUIRED_NODE_VERSION = 'v24.18.0';
export const LIFECYCLE_BASELINE = Object.freeze({
  kind: 'lifecycle-corrected-baseline',
  sourceCommit: 'b144106882fe244b19b738fc0acf3ffa07c7c9f3',
  nativeBindingSha256: '7b8863bb28aefd2e2eb7409f8be6dae57a252fe4a2688383007be7ea2f847bf7',
  distributionSha256: '1efffd0b63483e77cd2854fe716941000ae9548768691d7b5a64dceb011f3c45',
});
export const BASELINE_POOL_ENVIRONMENT = Object.freeze({
  ROLLDOWN_WORKER_THREADS: '18',
  RAYON_NUM_THREADS: '12',
  ROLLDOWN_MAX_BLOCKING_THREADS: '4',
});

export const PROJECTS = Object.freeze({
  'floating-vue': Object.freeze({
    id: 'floating-vue',
    band: 'small',
    repository: 'https://github.com/Akryum/floating-vue.git',
    commit: '19857764c4f73dea7ed44a7d970adb968ee7ad90',
    license: {
      path: 'LICENSE',
      sha256: '46e97d800fbd1540b43fed3720d378fa94a83226555438383a0fd26671470bf0',
      spdx: 'MIT',
    },
    entries: ['packages/floating-vue/src/index.ts'],
    sfcRoots: ['packages/floating-vue/src'],
    expectedPhysicalSfcCount: 4,
    expectedPhysicalSfcBytes: 12715,
    expectedPhysicalSfcManifestSha256:
      '5c41a77b5d8b397feb2bcad96d1551575787d509d7367653f8309dfec228c0b9',
    expectedReachedSfcCount: 4,
  }),
  'cabinet-icon': Object.freeze({
    id: 'cabinet-icon',
    band: 'medium',
    repository: 'https://github.com/cabinet-fe/icon.git',
    commit: '9cadad32c72d79424c75e3b6e56798f216bb0b06',
    license: {
      declarationPath: 'packages/vue/package.json',
      declarationField: 'license',
      declarationValue: 'MIT',
      note: 'The pinned repository does not contain a license file.',
    },
    entries: [
      'packages/vue/src/index.ts',
      'packages/vue/src/normal/index.ts',
      'packages/vue/src/colorful/index.ts',
      'packages/vue/src/names.ts',
    ],
    sfcRoots: ['packages/vue/src'],
    expectedPhysicalSfcCount: 166,
    expectedPhysicalSfcBytes: 109122,
    expectedPhysicalSfcManifestSha256:
      '9ae54c3311168ccd093c9da5a1e977c81654590ce040a5de63c2702ff0f3fedd',
    expectedReachedSfcCount: 166,
  }),
  primevue: Object.freeze({
    id: 'primevue',
    band: 'workspace-resolution-bridge',
    repository: 'https://github.com/primefaces/primevue.git',
    commit: 'd4374cb7c1267f35eba7cee5d0a266f50ca8ec84',
    license: {
      path: 'LICENSE.md',
      sha256: '39a2ce8d759cfcb59eccc49b0a417ad5c943f960c1bcdfba4720ca7547029af7',
      spdx: 'MIT',
    },
    entries: ['packages/primevue/src/index.js'],
    sfcRoots: ['packages/primevue'],
    expectedPhysicalSfcCount: 279,
    expectedPhysicalSfcBytes: 1728913,
    expectedPhysicalSfcManifestSha256:
      '64ade80952844967c9308036ef5b931ab565a96aabb0f927a4e01ff11a54bf4d',
    expectedReachedSfcCount: 275,
    knownUnreachedSfcPaths: [
      'packages/primevue/src/chart/BaseChart.vue',
      'packages/primevue/src/chart/Chart.vue',
      'packages/primevue/src/editor/BaseEditor.vue',
      'packages/primevue/src/editor/Editor.vue',
    ],
  }),
  gitlab: Object.freeze({
    id: 'gitlab',
    band: 'large-primary',
    repository: 'https://github.com/gitlabhq/gitlabhq.git',
    commit: '0ff224ddae1a652fffcee2f66ce3efc5fc816c03',
    license: {
      path: 'LICENSE',
      sha256: '62dfe4bdd76e08992c09cf335b2374b3e6acd4f2b959b4971760727d9b785ab4',
      spdx: 'MIT with project-specific additions; see pinned file',
    },
    entryGenerator: 'gitlab-production-generateEntries',
    sfcRoots: ['app/assets/javascripts'],
    expectedPhysicalSfcCount: 2620,
    expectedPhysicalSfcBytes: 10577454,
    expectedPhysicalSfcManifestSha256:
      'c9291d4515ce33e1654fcc37b32809e5cab5c2a701d597530494472428967cd3',
    sparsePaths: [
      '/app/assets/javascripts/**',
      '/app/assets/images/**',
      '/app/graphql/**',
      '/config/webpack.config.js',
      '/config/webpack.helpers.js',
      '/config/helpers/entry_points.js',
      '/config/helpers/aliases.js',
      '/config/helpers/context_aliases_shared.js',
      '/config/helpers/vue_version.js',
      '/config/helpers/vue3_infection_shared.js',
      '/config/plugins/webpack_vue3_infection_plugin.js',
      '/config/vue3migration/**',
      '/config/webpack.constants.js',
      '/package.json',
      '/yarn.lock',
      '/tsconfig*.json',
      '/babel.config.js',
      '/LICENSE',
      '/vendor/assets/javascripts/**',
    ],
    compilerContract: {
      vue2: '2.7.16',
      vue3Compat: '3.5.34',
      requiredQuery: '?vue3',
      sourcePins: Object.freeze({
        'package.json': '58ff42a3400c24e254cf6a6a127ea152d00133d8dc6a9e01c48f6f070517c8f9',
        'config/webpack.config.js':
          '910e9827cf1e34b8ea53038b5508c1242656e5c2abf4d3d1b598ff713b3116c8',
        'config/helpers/vue_version.js':
          'f1df8eabdfe541200c5e89fee7d07a770fb2619751b634a15563c186175835b4',
        'config/helpers/vue3_infection_shared.js':
          '51a7498e0296ce7437b065499c9def97140679814ff648c4ef856b800115588e',
        'config/helpers/context_aliases_shared.js':
          'edb7345caa65df77315ebda1dcd4ca832ce8d89d2a3c0a93432deceaf77de091',
        'config/plugins/webpack_vue3_infection_plugin.js':
          '673252aa4c0517638a2a9b065da1a7984428a55b7150c4ed3ac39ed60a2b3c83',
        'config/vue3migration/vue2_compiler.js':
          '0d44fd628caecee773749a2d199e749a9ba8115998f9f00cde1937b164d0fb96',
        'config/vue3migration/vue3_sfc_compiler.mjs':
          '16b810ff15652364d3ed274e849be988589e21557748f3a203d86c4c77e8ae46',
        'config/vue3migration/vue3_template_compiler.js':
          'ce3a2181558e47972876265a4e4e77220bf7b384b594839dca2543419b2b7384',
      }),
    },
  }),
  vben: Object.freeze({
    id: 'vben',
    band: 'large-fallback',
    fallbackFor: 'gitlab',
    repository: 'https://github.com/vbenjs/vue-vben-admin.git',
    commit: '8b7c245bc7a2346764d98d26003a2faf67a98182',
    license: {
      path: 'LICENSE',
      sha256: '26bd1c47f2d85139581c82c7b0197785322a11217c762d65f10c04f1567450ee',
      spdx: 'MIT',
    },
    entries: ['apps/web-antd/src/main.ts'],
    sfcRoots: ['.'],
    expectedPhysicalSfcCount: 680,
    expectedPhysicalSfcBytes: 1605262,
    expectedPhysicalSfcManifestSha256:
      '942f1f4be51fffabdaf48cc968437de51fddc01665a22ba5bdd7a2d05ccc7f61',
    minimumReachedSfcCount: 512,
    reachableEnvelopeRoots: ['apps/web-antd', 'packages'],
    expectedReachableEnvelopeSfcCount: 375,
    expectedObservedReachedSfcCount: 366,
    knownUnreachedEnvelopeSfcPaths: [
      'apps/web-antd/src/views/_core/fallback/coming-soon.vue',
      'apps/web-antd/src/views/_core/fallback/internal-error.vue',
      'apps/web-antd/src/views/_core/fallback/offline.vue',
      'packages/@core/ui-kit/form-ui/src/vben-form.vue',
      'packages/@core/ui-kit/shadcn-ui/src/ui/context-menu/ContextMenuPortal.vue',
      'packages/effects/layouts/src/widgets/preferences/icons/setting.vue',
      'packages/effects/plugins/src/tiptap/preview.vue',
      'packages/effects/plugins/src/tiptap/tiptap.vue',
      'packages/effects/plugins/src/vxe-table/use-vxe-grid.vue',
    ],
    dependencyPreparation: Object.freeze({
      packageManager: 'pnpm@11.7.0',
      rootLockfileSha256: '080206600c2dbf454e69f9bf7cd350a6dbb1458fe375842f19798c024e139b96',
      installLockfileSha256: '9e1d684ba4012783f182e09da128b569df162b9bb10ea1cc14a4cfe7ff6c968f',
      criticalPackages: Object.freeze([
        Object.freeze({
          path: 'apps/web-antd/node_modules/vue/package.json',
          version: '3.5.38',
          sha256: '7bb90bc1ad93ef2ce102d4252eaef820270eb44c80f4b1018c0f033642bac781',
        }),
        Object.freeze({
          path: 'packages/@core/ui-kit/form-ui/node_modules/unplugin-vue/package.json',
          version: '7.2.0',
          sha256: 'e5e394d8ace1faccb05048e3c9da899aab57ec39f92dc5ec6ab46ea684690815',
        }),
      ]),
    }),
  }),
  'tdesign-amendment-candidate': Object.freeze({
    id: 'tdesign-amendment-candidate',
    band: 'protocol-amendment-candidate-not-frozen',
    repository: 'https://github.com/Tencent/tdesign-vue-next.git',
    commit: 'dd334e2dc06d8ab48d1b6ebc5e9d4f6de67b16a2',
    license: {
      path: 'LICENSE',
      sha256: 'b3dbcb89dcf4a11abf1b70d043795a3da0c458af16fefd2ff315d9ff5875312f',
      spdx: 'MIT',
    },
    entries: ['packages/components/index.ts'],
    sfcRoots: ['packages/components'],
    expectedPhysicalSfcCount: 744,
    expectedPhysicalSfcBytes: 1121203,
    expectedPhysicalSfcManifestSha256:
      '244bdbd8599dccea2032e7a088a1f22a0cea1301494455899af952f81a554b77',
    minimumReachedSfcCount: 512,
    expectedObservedReachedSfcCount: 0,
    protocolStatus:
      'Reachability-only amendment candidate. It is not part of the frozen independent-project matrix.',
  }),
  'directus-amendment-candidate': Object.freeze({
    id: 'directus-amendment-candidate',
    band: 'protocol-amendment-candidate-not-frozen',
    repository: 'https://github.com/directus/directus.git',
    commit: '9f2f73aee7d8647d3f187dac43f724fe617763f5',
    license: {
      path: 'app/license',
      sha256: 'f209dfa60e56b29f6e8e5cefde91dec4ce86f12289209ede63520556385d555d',
      spdx: 'Business Source License 1.1; see pinned file',
    },
    entries: ['app/src/main.ts'],
    sfcRoots: ['.'],
    expectedPhysicalSfcCount: 561,
    expectedPhysicalSfcBytes: 2675339,
    expectedPhysicalSfcManifestSha256:
      '9c790f915893da23aa1406a6e8744a74a71cea4841a377ac20c5d803947d12a7',
    minimumReachedSfcCount: 512,
    expectedObservedReachedSfcCount: 546,
    knownUnreachedSfcPaths: [
      'app/src/components/v-tooltip.vue',
      'app/src/modules/content/components/version-promote-field.vue',
      'app/src/modules/settings/routes/flows/components/trigger-detail.vue',
      'app/src/modules/settings/routes/marketplace/routes/extension/components/extension-info-sidebar-detail.vue',
      'app/src/views/private/components/sidebar-button.vue',
      'packages/extensions-sdk/templates/display/javascript/source/display.vue',
      'packages/extensions-sdk/templates/display/typescript/source/display.vue',
      'packages/extensions-sdk/templates/interface/javascript/source/interface.vue',
      'packages/extensions-sdk/templates/interface/typescript/source/interface.vue',
      'packages/extensions-sdk/templates/layout/javascript/source/layout.vue',
      'packages/extensions-sdk/templates/layout/typescript/source/layout.vue',
      'packages/extensions-sdk/templates/module/javascript/source/module.vue',
      'packages/extensions-sdk/templates/module/typescript/source/module.vue',
      'packages/extensions-sdk/templates/panel/javascript/source/panel.vue',
      'packages/extensions-sdk/templates/panel/typescript/source/panel.vue',
    ],
    sparsePaths: [
      '/app/**',
      '/packages/**',
      '/sdk/**',
      '/package.json',
      '/pnpm-workspace.yaml',
      '/pnpm-lock.yaml',
      '/license',
      '/LICENSE*',
    ],
    protocolStatus:
      'Reachability-only amendment candidate. It is not part of the frozen independent-project matrix.',
    dependencyPreparation: Object.freeze({
      packageManager: 'pnpm@10.27.0',
      rootLockfileSha256: 'aeafd45f0650bd6265ada31e7a530f3a83ae62b7cce1c36eb60f45de9b2b42c1',
      installLockfileSha256: '560ff04b812fd10d7b6fa8a627164a4462c755f38fe848b490cf12923e54308a',
      criticalPackages: Object.freeze([
        Object.freeze({
          path: 'app/node_modules/vue/package.json',
          version: '3.5.24',
          sha256: '8d276c52619ee65010978b5ebb45b09abd8d11ba6b2ea00da3a28eec9d940700',
        }),
        Object.freeze({
          path: 'app/node_modules/@vitejs/plugin-vue/package.json',
          version: '6.0.1',
          sha256: '94f67bc6dc6da5236b313d819aecc703b486afcd720ec9e597bb03f4bc0417e6',
        }),
      ]),
    }),
  }),
});

export const PRIMARY_ADMISSION_ORDER = Object.freeze([
  'floating-vue',
  'cabinet-icon',
  'primevue',
  'gitlab',
]);

const CI_MARKERS = Object.freeze([
  'CI',
  'CONTINUOUS_INTEGRATION',
  'BUILD_NUMBER',
  'RUN_ID',
  'GITHUB_ACTIONS',
  'GITLAB_CI',
  'BUILDKITE',
  'CIRCLECI',
  'JENKINS_URL',
  'TEAMCITY_VERSION',
  'TF_BUILD',
  'TRAVIS',
]);

const isActiveMarker = (value) =>
  typeof value === 'string' &&
  value.length !== 0 &&
  value.toLowerCase() !== 'false' &&
  value !== '0';

export function assertLocalNode() {
  const active = CI_MARKERS.filter((name) => isActiveMarker(process.env[name]));
  if (active.length !== 0) {
    throw new Error(`independent Vue project runners refuse active CI: ${active.join(', ')}`);
  }
  if (process.version !== REQUIRED_NODE_VERSION) {
    throw new Error(
      `independent Vue project runners require Node.js ${REQUIRED_NODE_VERSION}, got ${process.version}`,
    );
  }
  if (process.env.NODE_OPTIONS !== undefined) {
    throw new Error('independent Vue project runners require unset inherited NODE_OPTIONS');
  }
}

export function projectDefinition(projectId) {
  const project = PROJECTS[projectId];
  if (!project) throw new Error(`unknown independent Vue project: ${projectId}`);
  return project;
}

export function projectRoot(projectId) {
  return nodePath.join(CORPUS_ROOT, projectId);
}
