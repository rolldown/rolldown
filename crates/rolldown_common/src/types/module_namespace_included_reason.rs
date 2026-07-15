use bitflags::bitflags;
bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct ModuleNamespaceIncludedReason: u8 {
        /// Fallback reason, all other reasons that we are not interested in.
        /// e.g.
        /// 1. a module is imported as namespace, and used the whole module namespace
        /// 2. A module is `require` by another module.
        const Unknown = 1;
        /// See `has_dynamic_exports` in [`rolldown::types::linking_metadata::LinkingMetadata`]
        /// See the normal-import branches in
        /// `stages/link_stage/passes/reference_needed_symbols.rs`.
        const ReExportDynamicExports = 1 << 1;

        const SimulateFacadeChunk = 1 << 2;
    }
}
