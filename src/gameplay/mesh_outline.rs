use avian3d::prelude::CollisionLayers;
use bevy::{
    prelude::*,
    render::render_resource::Face,
};

use super::{
    ColliderHierarchyChildOf, PhysLayer, Player,
    inventory::Item,
    props::Phone,
};
use crate::input::{IsHovering, UseRaycaster};

const OUTLINE_SCALE: f32 = 1.045;

pub(crate) struct MeshOutlinePlugin;

impl Plugin for MeshOutlinePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MeshOutlineMaterial>()
            .add_systems(
                Update,
                (
                    register_default_outline_targets,
                    ensure_outline_material,
                    spawn_outline_proxies,
                    update_outline_visibility,
                    cleanup_orphan_proxies,
                ),
            );
    }
}

/// add this to any world entity root that should be outline-highlightable (complex word huh)
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct MeshOutlineTarget;

#[derive(Component, Debug, Clone, Copy)]
struct MeshOutlineSourceBound;

#[derive(Component, Debug, Clone, Copy)]
struct MeshOutlineProxyMesh {
    root: Entity,
}

#[derive(Resource, Default)]
struct MeshOutlineMaterial {
    handle: Option<Handle<StandardMaterial>>,
}

fn register_default_outline_targets(
    mut commands: Commands,
    items: Query<Entity, (Added<Item>, Without<MeshOutlineTarget>)>,
    phones: Query<Entity, (Added<Phone>, Without<MeshOutlineTarget>)>,
) {
    for entity in &items {
        commands.entity(entity).insert(MeshOutlineTarget);
    }
    for entity in &phones {
        commands.entity(entity).insert(MeshOutlineTarget);
    }
}

fn ensure_outline_material(
    mut runtime: ResMut<MeshOutlineMaterial>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if runtime.handle.is_some() {
        return;
    }

    runtime.handle = Some(materials.add(StandardMaterial {
        base_color: Color::WHITE,
        emissive: Color::WHITE.into(),
        unlit: true,
        cull_mode: Some(Face::Front),
        ..default()
    }));
}

fn spawn_outline_proxies(
    mut commands: Commands,
    runtime: Res<MeshOutlineMaterial>,
    roots: Query<Entity, With<MeshOutlineTarget>>,
    children: Query<&Children>,
    meshes: Query<&Mesh3d>,
    sources_bound: Query<(), With<MeshOutlineSourceBound>>,
    proxies: Query<(), With<MeshOutlineProxyMesh>>,
) {
    let Some(material) = runtime.handle.as_ref() else {
        return;
    };

    for root in &roots {
        spawn_outline_proxy_for_mesh(
            &mut commands,
            &meshes,
            &sources_bound,
            &proxies,
            root,
            root,
            material,
        );

        for child in children.iter_descendants(root) {
            spawn_outline_proxy_for_mesh(
                &mut commands,
                &meshes,
                &sources_bound,
                &proxies,
                child,
                root,
                material,
            );
        }
    }
}

fn spawn_outline_proxy_for_mesh(
    commands: &mut Commands,
    meshes: &Query<&Mesh3d>,
    sources_bound: &Query<(), With<MeshOutlineSourceBound>>,
    proxies: &Query<(), With<MeshOutlineProxyMesh>>,
    entity: Entity,
    root: Entity,
    material: &Handle<StandardMaterial>,
) {
    if sources_bound.contains(entity) || proxies.contains(entity) {
        return;
    }
    let Ok(mesh) = meshes.get(entity) else {
        return;
    };

    commands.entity(entity).insert(MeshOutlineSourceBound);
    commands.entity(entity).with_children(|parent| {
        parent.spawn((
            Name::new("Mesh Outline Proxy"),
            MeshOutlineProxyMesh { root },
            Mesh3d(mesh.0.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_scale(Vec3::splat(OUTLINE_SCALE)),
            Visibility::Hidden,
        ));
    });
}

fn update_outline_visibility(
    q_hits: Query<&IsHovering, (With<UseRaycaster>, With<Player>)>,
    q_hierarchy: Query<&ColliderHierarchyChildOf>,
    q_layers: Query<&CollisionLayers>,
    targets: Query<(), With<MeshOutlineTarget>>,
    mut proxies: Query<(&MeshOutlineProxyMesh, &mut Visibility)>,
) {
    let focused_target = focused_outline_target(&q_hits, &q_hierarchy, &q_layers, &targets);

    for (proxy, mut visibility) in &mut proxies {
        *visibility = if Some(proxy.root) == focused_target {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

fn focused_outline_target(
    q_hits: &Query<&IsHovering, (With<UseRaycaster>, With<Player>)>,
    q_hierarchy: &Query<&ColliderHierarchyChildOf>,
    q_layers: &Query<&CollisionLayers>,
    targets: &Query<(), With<MeshOutlineTarget>>,
) -> Option<Entity> {
    let Ok(hits) = q_hits.single() else {
        return None;
    };
    let first = hits.0.iter().next()?;
    let candidate = q_hierarchy.get(*first).map_or(*first, |parent| parent.0);
    let Ok(layer) = q_layers.get(candidate) else {
        return None;
    };
    if !layer.memberships.has_all(PhysLayer::Usable) || targets.get(candidate).is_err() {
        return None;
    }
    Some(candidate)
}

fn cleanup_orphan_proxies(
    mut commands: Commands,
    proxies: Query<(Entity, &MeshOutlineProxyMesh)>,
    targets: Query<(), With<MeshOutlineTarget>>,
) {
    for (entity, proxy) in &proxies {
        if targets.get(proxy.root).is_err() {
            commands.entity(entity).despawn();
        }
    }
}
