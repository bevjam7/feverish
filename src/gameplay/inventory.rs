use avian3d::prelude::*;
use bevy::{
    ecs::{entity_disabling::Disabled, lifecycle::HookContext, world::DeferredWorld},
    prelude::*,
};
use bevy_trenchbroom::prelude::*;

use crate::{
    AssetServerExt, Usable,
    assets::ItemMeta,
    gameplay::{
        PlayerRoot,
        props::{Model, Prop},
    },
    input::Use,
    ui::{DiscoveryEntry, UiDiscoveryDb},
};

#[derive(Component, Default, Reflect)]
#[reflect(Component)]
#[relationship_target(relationship = InventoryItem)]
pub(crate) struct Inventory(Vec<Entity>);

#[derive(Component, Deref, Reflect)]
#[reflect(Component)]
#[component(on_add=Self::on_add_hook)]
#[relationship(relationship_target = Inventory)]
pub(crate) struct InventoryItem(pub(crate) Entity);

impl InventoryItem {
    fn on_add_hook(mut world: DeferredWorld, hook: HookContext) {
        world
            .commands()
            .entity(hook.entity)
            .insert(Disabled)
            .remove::<Transform>()
            .remove::<Collider>()
            .despawn_children();

        let model = world.get::<Model>(hook.entity).unwrap().clone();
        let Item { metadata } = world.get::<Item>(hook.entity).unwrap().clone();
        let assets = world.resource::<AssetServer>();
        let metadata_handle = assets
            // Attempt to get the item path automatically if not set
            .get_path_handle(metadata.unwrap_or_else(|| {
                let name = std::path::Path::new(&model.model)
                    .file_prefix()
                    .and_then(|stem| stem.to_str())
                    .unwrap();

                format!("items/{}.item.meta", name)
            }))
            .unwrap();
        let metadatas = world.resource::<Assets<ItemMeta>>();
        let metadata = metadatas.get(&metadata_handle).unwrap().clone();
        let mut discovery_db = world.resource_mut::<UiDiscoveryDb>();

        let into_id = |input: &String| {
            input
                .trim()
                .to_lowercase()
                .chars()
                .filter_map(|c| match (c.is_ascii_alphanumeric(), c.is_whitespace()) {
                    (true, _) => Some(c),
                    (_, true) => Some('-'),
                    _ => None,
                })
                .collect::<String>()
        };

        discovery_db.upsert(
            crate::ui::DiscoveryKind::Item,
            DiscoveryEntry::new(into_id(&metadata.name), metadata.name.clone())
                .subtitle(metadata.subtitle)
                .description(metadata.description)
                .model_path(format!("{}#Scene0", model.model))
                .seen(true),
        );
    }
}

#[point_class(base(Transform, Visibility, Prop))]
#[derive(Default, Clone)]
#[require(Usable)]
#[component(on_add=Self::on_add_hook)]
pub(crate) struct Item {
    /// Defaults to the specified GLTF name if not provided
    #[class(default = "items/name.item.meta")]
    pub(crate) metadata: Option<String>,
}

impl Item {
    fn on_add_hook(mut world: DeferredWorld, hook: HookContext) {
        if world.is_scene_world() {
            return;
        }

        world
            .commands()
            .entity(hook.entity)
            .observe(Self::add_to_inventory);
    }

    fn add_to_inventory(
        trigger: On<Use>,
        mut cmd: Commands,
        player: Single<Entity, With<PlayerRoot>>,
    ) {
        cmd.entity(trigger.0).insert(InventoryItem(player.entity()));
    }
}
