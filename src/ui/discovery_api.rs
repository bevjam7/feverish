use bevy::prelude::*;

use super::components::{
    DiscoveryEntry, DiscoveryInteraction, DiscoveryKind, UiDiscoveryCommand, UiDiscoveryDbSnapshot,
};

#[allow(dead_code)]
pub trait DiscoveryCommandsExt {
    fn upsert_discovery(&mut self, kind: DiscoveryKind, entry: DiscoveryEntry);
    fn upsert_discovery_item(&mut self, entry: DiscoveryEntry);
    fn upsert_discovery_npc(&mut self, entry: DiscoveryEntry);
    fn remove_discovery(&mut self, kind: DiscoveryKind, id: impl Into<String>);
    fn remove_discovery_item(&mut self, id: impl Into<String>);
    fn remove_discovery_npc(&mut self, id: impl Into<String>);
    fn set_discovery_seen(&mut self, kind: DiscoveryKind, id: impl Into<String>, seen: bool);
    fn set_discovery_item_seen(&mut self, id: impl Into<String>, seen: bool);
    fn set_discovery_npc_seen(&mut self, id: impl Into<String>, seen: bool);
    fn clear_discovery_items(&mut self);
    fn clear_discovery_npcs(&mut self);
    fn record_discovery_interaction(&mut self, interaction: DiscoveryInteraction);
    fn replace_discovery_db(&mut self, snapshot: UiDiscoveryDbSnapshot);
}

impl DiscoveryCommandsExt for Commands<'_, '_> {
    fn upsert_discovery(&mut self, kind: DiscoveryKind, entry: DiscoveryEntry) {
        self.write_message(UiDiscoveryCommand::Upsert { kind, entry });
    }

    fn upsert_discovery_item(&mut self, entry: DiscoveryEntry) {
        self.upsert_discovery(DiscoveryKind::Item, entry);
    }

    fn upsert_discovery_npc(&mut self, entry: DiscoveryEntry) {
        self.upsert_discovery(DiscoveryKind::Npc, entry);
    }

    fn remove_discovery(&mut self, kind: DiscoveryKind, id: impl Into<String>) {
        self.write_message(UiDiscoveryCommand::Remove {
            kind,
            id: id.into(),
        });
    }

    fn remove_discovery_item(&mut self, id: impl Into<String>) {
        self.remove_discovery(DiscoveryKind::Item, id);
    }

    fn remove_discovery_npc(&mut self, id: impl Into<String>) {
        self.remove_discovery(DiscoveryKind::Npc, id);
    }

    fn set_discovery_seen(&mut self, kind: DiscoveryKind, id: impl Into<String>, seen: bool) {
        self.write_message(UiDiscoveryCommand::SetSeen {
            kind,
            id: id.into(),
            seen,
        });
    }

    fn set_discovery_item_seen(&mut self, id: impl Into<String>, seen: bool) {
        self.set_discovery_seen(DiscoveryKind::Item, id, seen);
    }

    fn set_discovery_npc_seen(&mut self, id: impl Into<String>, seen: bool) {
        self.set_discovery_seen(DiscoveryKind::Npc, id, seen);
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

    fn record_discovery_interaction(&mut self, interaction: DiscoveryInteraction) {
        self.write_message(UiDiscoveryCommand::RecordInteraction { interaction });
    }

    fn replace_discovery_db(&mut self, snapshot: UiDiscoveryDbSnapshot) {
        self.write_message(UiDiscoveryCommand::ReplaceAll { snapshot });
    }
}
