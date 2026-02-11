use avian3d::prelude::{LinearVelocity, SpatialQuery, SpatialQueryFilter};
use bevy::{
    asset::{AssetPath, AssetServer},
    ecs::{lifecycle::HookContext, world::DeferredWorld},
    pbr::StandardMaterial,
    prelude::*,
};
use bevy_ahoy::{CharacterController, CharacterControllerState};
use bevy_seedling::{
    prelude::{HrtfNode, Volume},
    sample::{AudioSample, PlaybackSettings, SamplePlayer},
};
use bevy_trenchbroom::prelude::*;
use rand::RngExt;

use crate::{
    AssetServerExt,
    audio::mixer::WorldSfxPool,
    gameplay::{ColliderHierarchyChildOf, PlayerRoot},
    psx::PsxPbrMaterial,
};

#[point_class(
    group("sound"),
    classname("point"),
    base(Transform),
    iconsprite({ path: "sprites/audio_emitter.png", scale: 0.125 }),
)]
#[derive(Clone)]
#[component(on_add=Self::on_add_hook)]
pub struct SoundPoint {
    pub(crate) volume: f32,
    #[class(default = "audio/sound.ogg", must_set)]
    pub(crate) sample: String,
    pub(crate) repeat: bool,
    pub(crate) play_immediately: bool,
    pub(crate) repeat_count: Option<usize>,
}

impl Default for SoundPoint {
    fn default() -> Self {
        Self {
            volume: 1.0,
            sample: Default::default(),
            repeat: true,
            play_immediately: true,
            repeat_count: None,
        }
    }
}

impl SoundPoint {
    fn on_add_hook(mut world: DeferredWorld, hook: HookContext) {
        if world.is_scene_world() {
            return;
        }
        let point = world.get::<Self>(hook.entity).unwrap();
        let assets = world.resource::<AssetServer>();
        let sample = assets
            .get_path_handle::<AudioSample>(point.sample.clone())
            .unwrap();

        let volume = point.volume;
        let mut sampler =
            SamplePlayer::new(sample).with_volume(bevy_seedling::prelude::Volume::Linear(volume));

        if point.repeat {
            sampler.repeat_mode = match point.repeat_count {
                Some(count) => bevy_seedling::prelude::RepeatMode::RepeatMultiple {
                    num_times_to_repeat: count as u32,
                },
                None => bevy_seedling::prelude::RepeatMode::RepeatEndlessly,
            };
        }

        let mut playback_settings = PlaybackSettings::default();
        if !point.play_immediately {
            playback_settings.pause();
        }

        world.commands().entity(hook.entity).insert((
            sampler,
            playback_settings,
            bevy_seedling::sample_effects![HrtfNode::default()],
            WorldSfxPool,
        ));
    }
}

#[derive(Component, Reflect, Default, Clone, Copy, PartialEq, Eq, Debug)]
#[reflect(Component)]
pub(crate) enum Surface {
    #[default]
    Concrete,
    Wood,
    Grass,
    Dirt,
}

#[derive(Component)]
pub(crate) struct FootstepPlayer {
    pub(crate) stride_length: f32,
    pub(crate) distance_overflow: f32,
    pub(crate) base_volume: f32,
    pub(crate) active_surface: Surface,
    active_material_name: String,
    last_logged_material_name: String,
    concrete_samples: Vec<Handle<AudioSample>>,
    wood_samples: Vec<Handle<AudioSample>>,
    grass_samples: Vec<Handle<AudioSample>>,
    dirt_samples: Vec<Handle<AudioSample>>,
}

impl Default for FootstepPlayer {
    fn default() -> Self {
        Self {
            // base step distance at regular walk speed
            stride_length: 1.1,
            distance_overflow: 0.0,
            base_volume: 0.45,
            active_surface: Surface::Concrete,
            active_material_name: String::new(),
            last_logged_material_name: String::new(),
            concrete_samples: Vec::new(),
            wood_samples: Vec::new(),
            grass_samples: Vec::new(),
            dirt_samples: Vec::new(),
        }
    }
}

impl FootstepPlayer {
    fn samples_for_surface(&self, surface: Surface) -> &[Handle<AudioSample>] {
        match surface {
            Surface::Concrete => &self.concrete_samples,
            Surface::Wood => &self.wood_samples,
            Surface::Grass => &self.grass_samples,
            Surface::Dirt => &self.dirt_samples,
        }
    }
}

pub(super) fn default_footstep_player(assets: &AssetServer) -> FootstepPlayer {
    let mut footsteps = FootstepPlayer::default();
    footsteps.stride_length = 1.1;
    footsteps.base_volume = 0.5;
    footsteps.concrete_samples = load_surface_samples(assets, "concrete");
    footsteps.wood_samples = load_surface_samples(assets, "wood");
    footsteps.grass_samples = load_surface_samples(assets, "grass");
    footsteps.dirt_samples = load_surface_samples(assets, "dirt");
    footsteps
}

fn load_surface_samples(assets: &AssetServer, surface_name: &str) -> Vec<Handle<AudioSample>> {
    (1..=4)
        .map(|index| {
            let path = format!("audio/footsteps/{surface_name}_{index:02}.ogg");
            assets.load(path)
        })
        .collect()
}

pub(super) fn detect_footstep_surface(
    spatial_query: SpatialQuery,
    hierarchy: Query<&ColliderHierarchyChildOf>,
    children: Query<&Children>,
    surface_query: Query<&Surface>,
    psx_material_handles: Query<&MeshMaterial3d<PsxPbrMaterial>>,
    standard_material_handles: Query<&MeshMaterial3d<StandardMaterial>>,
    psx_materials: Res<Assets<PsxPbrMaterial>>,
    standard_materials: Res<Assets<StandardMaterial>>,
    assets: Res<AssetServer>,
    mut footsteps: Query<
        (
            Entity,
            &GlobalTransform,
            &CharacterControllerState,
            &mut FootstepPlayer,
        ),
        With<PlayerRoot>,
    >,
) {
    for (player, player_transform, controller, mut footstep_player) in &mut footsteps {
        if controller.grounded.is_none() {
            footstep_player.active_surface = Surface::Concrete;
            footstep_player.active_material_name = "air".to_owned();
            continue;
        }

        let ray_origin = player_transform.translation() + Vec3::Y * 0.1;
        let filter = SpatialQueryFilter::from_excluded_entities([player]);
        let hit = spatial_query.cast_ray(ray_origin, Dir3::NEG_Y, 1.6, true, &filter);

        let Some(hit) = hit else {
            footstep_player.active_surface = Surface::Concrete;
            footstep_player.active_material_name = "default_concrete".to_owned();
            continue;
        };

        let target = hierarchy
            .get(hit.entity)
            .ok()
            .map_or(hit.entity, |parent| parent.0);

        let explicit_surface = surface_query
            .get(target)
            .ok()
            .or_else(|| surface_query.get(hit.entity).ok())
            .copied();

        if let Some(surface) = explicit_surface {
            footstep_player.active_surface = surface;
            footstep_player.active_material_name = format!("explicit_{}", surface_name(surface));
            log_footstep_material_change(player, &mut footstep_player);
            continue;
        }

        // fallback: infer from the actual material texture name
        // this is really messy hah
        let inferred = infer_surface_from_entity_material(
            target,
            &children,
            &psx_material_handles,
            &standard_material_handles,
            &psx_materials,
            &standard_materials,
            &assets,
        )
        .or_else(|| {
            infer_surface_from_entity_material(
                hit.entity,
                &children,
                &psx_material_handles,
                &standard_material_handles,
                &psx_materials,
                &standard_materials,
                &assets,
            )
        })
        .unwrap_or((Surface::Concrete, "default_concrete".to_owned()));

        footstep_player.active_surface = inferred.0;
        footstep_player.active_material_name = inferred.1;
        log_footstep_material_change(player, &mut footstep_player);
    }
}

pub(super) fn handle_footsteps(
    mut commands: Commands,
    time: Res<Time>,
    mut footsteps: Query<
        (
            &GlobalTransform,
            &LinearVelocity,
            &CharacterController,
            &CharacterControllerState,
            &mut FootstepPlayer,
        ),
        With<PlayerRoot>,
    >,
) {
    let mut rng = rand::rng();
    let dt = time.delta_secs();

    for (transform, velocity, cfg, controller, mut footstep_player) in &mut footsteps {
        if controller.grounded.is_none() {
            footstep_player.distance_overflow = 0.0;
            continue;
        }

        let speed = Vec2::new(velocity.x, velocity.z).length();
        if speed <= 0.03 {
            footstep_player.distance_overflow = (footstep_player.distance_overflow - dt).max(0.0);
            continue;
        }

        footstep_player.distance_overflow += speed * dt;
        let stride_length = stride_length_for_speed(speed, cfg, controller, &footstep_player);

        let mut emitted_this_frame = 0_u8;
        while footstep_player.distance_overflow >= stride_length && emitted_this_frame < 2 {
            footstep_player.distance_overflow -= stride_length;

            let samples = footstep_player.samples_for_surface(footstep_player.active_surface);
            let Some(sample) = select_random_step(samples, &mut rng) else {
                break;
            };

            let (min_pitch, max_pitch) = surface_pitch_range(footstep_player.active_surface);
            let pitch = rng.random_range(min_pitch..max_pitch);
            let volume =
                footstep_player.base_volume * surface_volume_scale(footstep_player.active_surface);
            commands.spawn((
                Name::new("footstep_sfx"),
                Transform::from_translation(transform.translation()),
                SamplePlayer::new(sample.clone()).with_volume(Volume::Linear(volume)),
                PlaybackSettings::default().with_speed(pitch).despawn(),
                WorldSfxPool,
            ));
            info!(
                "footstep step surface={} material={} speed={:.2}",
                surface_name(footstep_player.active_surface),
                footstep_player.active_material_name,
                speed
            );
            emitted_this_frame += 1;
        }
    }
}

fn select_random_step<'a>(
    samples: &'a [Handle<AudioSample>],
    rng: &mut rand::rngs::ThreadRng,
) -> Option<&'a Handle<AudioSample>> {
    if samples.is_empty() {
        return None;
    }
    let index = rng.random_range(0..samples.len());
    Some(&samples[index])
}

#[solid_class(group("surface"), classname("surface_concrete"), base(Transform))]
#[derive(Component, Clone, Copy, Default)]
#[component(on_add=Self::on_add_hook)]
pub(crate) struct SurfaceConcrete;

impl SurfaceConcrete {
    fn on_add_hook(mut world: DeferredWorld, hook: HookContext) {
        set_surface_on_add(&mut world, hook.entity, Surface::Concrete);
    }
}

#[solid_class(group("surface"), classname("surface_wood"), base(Transform))]
#[derive(Component, Clone, Copy, Default)]
#[component(on_add=Self::on_add_hook)]
pub(crate) struct SurfaceWood;

impl SurfaceWood {
    fn on_add_hook(mut world: DeferredWorld, hook: HookContext) {
        set_surface_on_add(&mut world, hook.entity, Surface::Wood);
    }
}

#[solid_class(group("surface"), classname("surface_grass"), base(Transform))]
#[derive(Component, Clone, Copy, Default)]
#[component(on_add=Self::on_add_hook)]
pub(crate) struct SurfaceGrass;

impl SurfaceGrass {
    fn on_add_hook(mut world: DeferredWorld, hook: HookContext) {
        set_surface_on_add(&mut world, hook.entity, Surface::Grass);
    }
}

#[solid_class(group("surface"), classname("surface_dirt"), base(Transform))]
#[derive(Component, Clone, Copy, Default)]
#[component(on_add=Self::on_add_hook)]
pub(crate) struct SurfaceDirt;

impl SurfaceDirt {
    fn on_add_hook(mut world: DeferredWorld, hook: HookContext) {
        set_surface_on_add(&mut world, hook.entity, Surface::Dirt);
    }
}

fn set_surface_on_add(world: &mut DeferredWorld, entity: Entity, surface: Surface) {
    if world.is_scene_world() {
        return;
    }
    world.commands().entity(entity).insert(surface);
}

fn stride_length_for_speed(
    speed: f32,
    cfg: &CharacterController,
    state: &CharacterControllerState,
    footsteps: &FootstepPlayer,
) -> f32 {
    let walk_speed = cfg.speed.max(0.1);
    let speed_ratio = (speed / walk_speed).clamp(0.25, 1.6);
    let mut stride = footsteps.stride_length * (0.72 + 0.38 * speed_ratio);
    if state.crouching {
        stride *= 0.84;
    }
    stride.clamp(0.55, 1.75)
}

fn surface_volume_scale(surface: Surface) -> f32 {
    match surface {
        Surface::Concrete => 1.0,
        Surface::Wood => 0.9,
        Surface::Grass => 0.8,
        Surface::Dirt => 0.86,
    }
}

fn infer_surface_from_entity_material(
    entity: Entity,
    children: &Query<&Children>,
    psx_material_handles: &Query<&MeshMaterial3d<PsxPbrMaterial>>,
    standard_material_handles: &Query<&MeshMaterial3d<StandardMaterial>>,
    psx_materials: &Assets<PsxPbrMaterial>,
    standard_materials: &Assets<StandardMaterial>,
    assets: &AssetServer,
) -> Option<(Surface, String)> {
    let texture_name = texture_name_for_entity(
        entity,
        children,
        psx_material_handles,
        standard_material_handles,
        psx_materials,
        standard_materials,
        assets,
    )?;
    Some((surface_from_texture_name(&texture_name), texture_name))
}

fn texture_name_for_entity(
    entity: Entity,
    children: &Query<&Children>,
    psx_material_handles: &Query<&MeshMaterial3d<PsxPbrMaterial>>,
    standard_material_handles: &Query<&MeshMaterial3d<StandardMaterial>>,
    psx_materials: &Assets<PsxPbrMaterial>,
    standard_materials: &Assets<StandardMaterial>,
    assets: &AssetServer,
) -> Option<String> {
    if let Some(name) =
        texture_name_from_psx_material(entity, psx_material_handles, psx_materials, assets)
    {
        return Some(name);
    }
    if let Some(name) = texture_name_from_standard_material(
        entity,
        standard_material_handles,
        standard_materials,
        assets,
    ) {
        return Some(name);
    }

    for child in children.iter_descendants(entity) {
        if let Some(name) =
            texture_name_from_psx_material(child, psx_material_handles, psx_materials, assets)
        {
            return Some(name);
        }
        if let Some(name) = texture_name_from_standard_material(
            child,
            standard_material_handles,
            standard_materials,
            assets,
        ) {
            return Some(name);
        }
    }
    None
}

fn texture_name_from_psx_material(
    entity: Entity,
    handles: &Query<&MeshMaterial3d<PsxPbrMaterial>>,
    materials: &Assets<PsxPbrMaterial>,
    assets: &AssetServer,
) -> Option<String> {
    let handle = handles.get(entity).ok()?;
    let material = materials.get(&handle.0)?;
    let texture = material.base.base_color_texture.as_ref()?;
    texture_name_from_handle(texture, assets)
}

fn texture_name_from_standard_material(
    entity: Entity,
    handles: &Query<&MeshMaterial3d<StandardMaterial>>,
    materials: &Assets<StandardMaterial>,
    assets: &AssetServer,
) -> Option<String> {
    let handle = handles.get(entity).ok()?;
    let material = materials.get(&handle.0)?;
    let texture = material.base_color_texture.as_ref()?;
    texture_name_from_handle(texture, assets)
}

fn texture_name_from_handle(texture: &Handle<Image>, assets: &AssetServer) -> Option<String> {
    let path = assets.get_path(texture.id())?;
    path.path().file_stem()?.to_str().map(ToOwned::to_owned)
}

fn surface_from_texture_name(name: &str) -> Surface {
    // mpping mirrors names in `assets_source/materials`
    // maybe this is too manual but shhould work?
    match name {
        "wood_floor" | "wood_paneling" => Surface::Wood,
        "grass" | "carpet" | "carpet_b" => Surface::Grass,
        "dirt" => Surface::Dirt,
        _ => Surface::Concrete,
    }
}

fn surface_pitch_range(surface: Surface) -> (f64, f64) {
    match surface {
        Surface::Concrete => (0.94, 1.06),
        // lower and tighter pitch range reads less "clicky concrete", more woody thump
        Surface::Wood => (0.82, 0.94),
        Surface::Grass => (0.90, 1.00),
        Surface::Dirt => (0.88, 1.00),
    }
}

fn surface_name(surface: Surface) -> &'static str {
    match surface {
        Surface::Concrete => "concrete",
        Surface::Wood => "wood",
        Surface::Grass => "grass",
        Surface::Dirt => "dirt",
    }
}

fn log_footstep_material_change(player: Entity, footstep_player: &mut FootstepPlayer) {
    if footstep_player.last_logged_material_name == footstep_player.active_material_name {
        return;
    }
    footstep_player.last_logged_material_name = footstep_player.active_material_name.clone();
    info!(
        "footstep debug player={:?} surface={} material={}",
        player,
        surface_name(footstep_player.active_surface),
        footstep_player.active_material_name
    );
}
