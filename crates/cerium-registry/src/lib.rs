//! Static and data-driven registries, block states, tags and datapacks
//! (roadmap steps 27-32).
//!
//! The tables in [`vanilla`] are generated at build time from vanilla's data
//! generator reports (roadmap step 3).

pub mod vanilla {
    //! Facts generated from the pinned vanilla version's data generator
    //! reports.

    /// Summary of one built-in registry from the vanilla registry report.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct RegistryInfo {
        /// Registry identifier, for example `minecraft:block`.
        pub name: &'static str,
        /// The registry's own network ID.
        pub protocol_id: u32,
        /// The default entry, if the registry has one.
        pub default: Option<&'static str>,
        /// Number of entries in the registry.
        pub entry_count: usize,
    }

    include!(concat!(env!("OUT_DIR"), "/vanilla_reports.rs"));
}

#[cfg(test)]
mod tests {
    use super::vanilla::{BLOCK_COUNT, BLOCK_STATE_COUNT, REGISTRIES, RegistryInfo};

    fn registry(name: &str) -> &'static RegistryInfo {
        REGISTRIES
            .iter()
            .find(|registry| registry.name == name)
            .unwrap_or_else(|| panic!("registry {name} is missing"))
    }

    #[test]
    fn block_registry_matches_block_report() {
        let blocks = registry("minecraft:block");
        assert_eq!(blocks.entry_count, BLOCK_COUNT);
        assert_eq!(blocks.default, Some("minecraft:air"));
    }

    #[test]
    fn registries_are_sorted_unique_and_namespaced() {
        let names: Vec<&str> = REGISTRIES.iter().map(|info| info.name).collect();
        assert!(names.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(names.iter().all(|name| name.starts_with("minecraft:")));
    }

    #[test]
    fn registry_protocol_ids_cover_zero_to_count() {
        let mut ids: Vec<u32> = REGISTRIES.iter().map(|info| info.protocol_id).collect();
        ids.sort_unstable();
        assert!(ids.iter().copied().eq(0..REGISTRIES.len() as u32));
    }

    #[test]
    fn every_block_has_at_least_one_state() {
        assert!(BLOCK_STATE_COUNT >= BLOCK_COUNT);
    }
}
