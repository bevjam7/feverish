use bevy::prelude::*;

use super::components::{DiscoveryEntry, DiscoveryKind, UiDiscoveryCommand};

#[allow(dead_code)]
pub trait DiscoveryCommandsExt {
    fn upsert_discovery_item(&mut self, entry: DiscoveryEntry);
    fn upsert_discovery_npc(&mut self, entry: DiscoveryEntry);
    fn remove_discovery_item(&mut self, id: impl Into<String>);
    fn remove_discovery_npc(&mut self, id: impl Into<String>);
    fn set_discovery_item_seen(&mut self, id: impl Into<String>, seen: bool);
    fn set_discovery_npc_seen(&mut self, id: impl Into<String>, seen: bool);
    fn clear_discovery_items(&mut self);
    fn clear_discovery_npcs(&mut self);
}

impl DiscoveryCommandsExt for Commands<'_, '_> {
    fn upsert_discovery_item(&mut self, entry: DiscoveryEntry) {
        self.write_message(UiDiscoveryCommand::Upsert {
            kind: DiscoveryKind::Item,
            entry,
        });
    }

    fn upsert_discovery_npc(&mut self, entry: DiscoveryEntry) {
        self.write_message(UiDiscoveryCommand::Upsert {
            kind: DiscoveryKind::Npc,
            entry,
        });
    }

    fn remove_discovery_item(&mut self, id: impl Into<String>) {
        self.write_message(UiDiscoveryCommand::Remove {
            kind: DiscoveryKind::Item,
            id: id.into(),
        });
    }

    fn remove_discovery_npc(&mut self, id: impl Into<String>) {
        self.write_message(UiDiscoveryCommand::Remove {
            kind: DiscoveryKind::Npc,
            id: id.into(),
        });
    }

    fn set_discovery_item_seen(&mut self, id: impl Into<String>, seen: bool) {
        self.write_message(UiDiscoveryCommand::SetSeen {
            kind: DiscoveryKind::Item,
            id: id.into(),
            seen,
        });
    }

    fn set_discovery_npc_seen(&mut self, id: impl Into<String>, seen: bool) {
        self.write_message(UiDiscoveryCommand::SetSeen {
            kind: DiscoveryKind::Npc,
            id: id.into(),
            seen,
        });
    }

    fn clear_discovery_items(&mut self) {
        self.write_message(UiDiscoveryCommand::ClearKind {
            kind: DiscoveryKind::Item,
        });
    }

    fn clear_discovery_npcs(&mut self) {
        self.write_message(UiDiscoveryCommand::ClearKind {
            kind: DiscoveryKind::Npc,
        });
    }
}
