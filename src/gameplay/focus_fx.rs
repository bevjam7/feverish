use std::collections::HashSet;

use avian3d::prelude::CollisionLayers;
use bevy::prelude::*;

use crate::{
    gameplay::{ColliderHierarchyChildOf, PhysLayer, Player},
    input::{IsHovering, UseRaycaster},
    psx::{PsxCamera, PsxConfig, PsxPbrMaterial, set_material_focused},
};

#[derive(Default)]
pub(super) struct FocusFxState {
    saturation: f32,
    focused_materials: HashSet<AssetId<PsxPbrMaterial>>,
    initialized: bool,
    focus_grace_timer: f32,
}

pub(super) fn handle_focus_effect(
    mut q_config: Query<&mut PsxConfig, With<PsxCamera>>,
    q_hits: Query<&IsHovering, (With<UseRaycaster>, With<Player>)>,
    q_layers: Query<&CollisionLayers>,
    q_hierarchy: Query<&ColliderHierarchyChildOf>,
    q_children: Query<&Children>,
    q_materials: Query<&MeshMaterial3d<PsxPbrMaterial>>,
    mut materials: ResMut<Assets<PsxPbrMaterial>>,
    time: Res<Time>,
    mut state: Local<FocusFxState>,
) {
    let Ok(mut config) = q_config.single_mut() else {
        return;
    };

    if !state.initialized {
        state.saturation = config.saturation.clamp(0.0, 1.0);
        state.initialized = true;
    }

    let focused_entity = focused_usable_entity(&q_hits, &q_hierarchy, &q_layers);
    let focused_materials = focused_entity
        .map(|entity| {
            let mut handles = collect_material_handles(entity, &q_materials, &q_children);
            if handles.is_empty()
                && let Ok(parent) = q_hierarchy.get(entity)
            {
                handles = collect_material_handles(parent.0, &q_materials, &q_children);
            }
            handles
        })
        .unwrap_or_default();

    if state.focused_materials != focused_materials {
        for old in state.focused_materials.difference(&focused_materials) {
            if let Some(mat) = materials.get_mut(*old) {
                set_material_focused(mat, false);
            }
        }

        for new_focus in focused_materials.difference(&state.focused_materials) {
            if let Some(mat) = materials.get_mut(*new_focus) {
                set_material_focused(mat, true);
            }
        }

        state.focused_materials = focused_materials;
    }

    if focused_entity.is_some() {
        state.focus_grace_timer = 0.10;
    } else {
        state.focus_grace_timer = (state.focus_grace_timer - time.delta_secs()).max(0.0);
    }

    let should_desaturate = focused_entity.is_some() || state.focus_grace_timer > 0.0;
    let target_saturation = if should_desaturate { 0.0 } else { 1.0 };
    let rate = if should_desaturate { 2.8 } else { 1.7 };
    let t = 1.0 - (-rate * time.delta_secs()).exp();
    state.saturation += (target_saturation - state.saturation) * t;
    let next = state.saturation.clamp(0.0, 1.0);
    if (config.saturation - next).abs() > 0.001 {
        config.saturation = next;
    }
}

fn focused_usable_entity(
    q_hits: &Query<&IsHovering, (With<UseRaycaster>, With<Player>)>,
    q_hierarchy: &Query<&ColliderHierarchyChildOf>,
    q_layers: &Query<&CollisionLayers>,
) -> Option<Entity> {
    let Ok(hits) = q_hits.single() else {
        return None;
    };
    let first = hits.0.iter().next()?;
    let target = q_hierarchy.get(*first).ok().map_or(first, |h| &h.0);
    let layer = q_layers.get(*target).ok()?;
    if layer.memberships.has_all(PhysLayer::Usable) {
        Some(*target)
    } else {
        None
    }
}

fn collect_material_handles(
    entity: Entity,
    q_materials: &Query<&MeshMaterial3d<PsxPbrMaterial>>,
    q_children: &Query<&Children>,
) -> HashSet<AssetId<PsxPbrMaterial>> {
    let mut handles = HashSet::new();

    if let Ok(material) = q_materials.get(entity) {
        handles.insert(material.id());
    }

    for child in q_children.iter_descendants(entity) {
        if let Ok(material) = q_materials.get(child) {
            handles.insert(material.id());
        }
    }

    handles
}
