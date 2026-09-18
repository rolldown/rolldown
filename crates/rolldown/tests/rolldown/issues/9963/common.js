// The shared "base" module — imported by the entry and reachable from every
// lazy route. This is the module that gets split into its own chunk on
// rolldown 1.0.3 (the analogue of the Vuetify build's `VDivider-*.js`).
export const common = 'common-base';
