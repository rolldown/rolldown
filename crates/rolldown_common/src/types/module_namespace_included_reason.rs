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
        /// 1. https://github.com/rolldown/rolldown/blob/8bc7dca5a09047b6b494e3fa7b6b7564aa465372/crates/rolldown/src/stages/link_stage/reference_needed_symbols.rs?plain=1#L122-L134
        /// 2. https://github.com/rolldown/rolldown/blob/8bc7dca5a09047b6b494e3fa7b6b7564aa465372/crates/rolldown/src/stages/link_stage/reference_needed_symbols.rs?plain=1#L188-L197
        const ReExportExternalModule = 1 << 1;
    }
}
