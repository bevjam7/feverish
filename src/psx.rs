use bevy::{
    asset::uuid::Uuid,
    camera::{RenderTarget, visibility::RenderLayers},
    image::ImageSampler,
    input::{ButtonState, mouse::MouseButtonInput},
    pbr::{ExtendedMaterial, MaterialExtension},
    picking::pointer::{
        Location, PointerAction, PointerButton, PointerId, PointerInput, PointerLocation,
    },
    prelude::*,
    render::render_resource::{AsBindGroup, Extent3d, TextureFormat, TextureUsages},
    scene::{SceneInstance, SceneSpawner},
    shader::ShaderRef,
    window::PrimaryWindow,
};

const PSX_CANVAS_LAYER: usize = 30;
const INTERNAL_WIDTH: u32 = 960;
const INTERNAL_HEIGHT: u32 = 540;
const FX_SNAP: u32 = 1;
const FX_DITHER: u32 = 2;
const FX_QUANTIZE: u32 = 4;
const FX_AFFINE: u32 = 8;
pub(crate) const FX_FOCUSED: u32 = 16;
const SHADER_PSX_FOCUS_EXT: &str = "shaders/psx_focus_extended.wgsl";
const PSX_POINTER_UUID: u128 = 0x4a3e_0b7a_8b9a_4a61_9a37_ba2a_41b2_2dd1;

#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct PsxConfig {
    pub resolution: UVec2,
    pub snap: bool,
    pub dither: bool,
    pub quantize: bool,
    pub affine: bool,
    pub quantize_steps: u32,
    pub dither_strength: f32,
    pub dither_scale: f32,
    pub dither_mode: PsxDitherMode,
    pub saturation: f32,
}

impl Default for PsxConfig {
    fn default() -> Self {
        Self {
            resolution: UVec2::new(INTERNAL_WIDTH, INTERNAL_HEIGHT),
            snap: true,
            dither: true,
            quantize: true,
            affine: true,
            quantize_steps: 160,
            dither_strength: 0.18,
            dither_scale: 1.8,
            dither_mode: PsxDitherMode::Bayer4,
            saturation: 1.0,
        }
    }
}

#[derive(Component, Debug, Default, Clone, Copy)]
pub(crate) struct PsxCamera;

#[derive(Component, Debug, Clone)]
pub(crate) struct Psxify;

#[derive(Component, Debug, Default)]
pub(crate) struct PsxWorldRoot;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub(crate) enum PsxDitherMode {
    Bayer4 = 0,
    Ign = 3,
    Hash = 4,
}

#[derive(Resource)]
struct PsxRenderTarget {
    texture: Handle<Image>,
}

#[derive(Component)]
struct PsxLowResCanvas;

#[derive(Component, Default)]
struct PsxSceneTagged;

#[derive(Component)]
struct PsxConverted;

#[derive(Component)]
struct PsxPickingPointer;

pub(crate) type PsxPbrMaterial = ExtendedMaterial<StandardMaterial, PsxExt>;

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub(crate) struct PsxExt {
    #[uniform(100)]
    resolution: Vec2,
    #[uniform(100)]
    quantize_steps: u32,
    #[uniform(100)]
    pub(crate) flags: u32,
    #[uniform(100)]
    dither_strength: f32,
    #[uniform(100)]
    dither_scale: f32,
    #[uniform(100)]
    dither_mode: u32,
    #[uniform(100)]
    saturation: f32,
}

pub(crate) fn set_material_focused(material: &mut PsxPbrMaterial, focused: bool) {
    if focused {
        material.extension.flags |= FX_FOCUSED;
    } else {
        material.extension.flags &= !FX_FOCUSED;
    }
}

impl MaterialExtension for PsxExt {
    fn fragment_shader() -> ShaderRef {
        SHADER_PSX_FOCUS_EXT.into()
    }
}

pub(crate) struct PsxPlugin;

impl Plugin for PsxPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<PsxPbrMaterial>::default())
            .add_systems(PreStartup, setup_render_target)
            .add_systems(
                Startup,
                (
                    spawn_framebuffer_sprite,
                    spawn_present_camera,
                    spawn_psx_pointer,
                ),
            )
            .add_systems(
                Update,
                (
                    tag_scene_descendants_with_psxify,
                    swap_standard_to_extended,
                    apply_psx_from_camera,
                    attach_psx_camera_to_target,
                    resize_render_target_from_camera,
                    fit_canvas_to_window,
                    update_psx_pointer_location,
                    forward_mouse_buttons_to_pointer,
                ),
            );
    }
}

fn flags_from(cfg: PsxConfig) -> u32 {
    [
        (FX_SNAP, cfg.snap),
        (FX_DITHER, cfg.dither),
        (FX_QUANTIZE, cfg.quantize),
        (FX_AFFINE, cfg.affine),
    ]
    .into_iter()
    .fold(0u32, |acc, (bit, on)| if on { acc | bit } else { acc })
}

fn make_ext(cfg: PsxConfig) -> PsxExt {
    PsxExt {
        resolution: Vec2::new(cfg.resolution.x as f32, cfg.resolution.y as f32),
        quantize_steps: cfg.quantize_steps.max(2),
        flags: flags_from(cfg),
        dither_strength: cfg.dither_strength.max(0.0),
        dither_scale: cfg.dither_scale.max(0.25),
        dither_mode: cfg.dither_mode as u32,
        saturation: cfg.saturation.clamp(0.0, 1.0),
    }
}

fn setup_render_target(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let mut image = Image::new_target_texture(
        INTERNAL_WIDTH,
        INTERNAL_HEIGHT,
        TextureFormat::Bgra8UnormSrgb,
        None,
    );
    image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    image.sampler = ImageSampler::nearest();
    let texture = images.add(image);
    commands.insert_resource(PsxRenderTarget { texture });
}

fn spawn_framebuffer_sprite(mut commands: Commands, target: Res<PsxRenderTarget>) {
    commands.spawn((
        Name::new("PSX Canvas"),
        Sprite {
            image: target.texture.clone(),
            custom_size: Some(Vec2::new(INTERNAL_WIDTH as f32, INTERNAL_HEIGHT as f32)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 0.0),
        RenderLayers::layer(PSX_CANVAS_LAYER),
        PsxLowResCanvas,
    ));
}

fn spawn_present_camera(mut commands: Commands) {
    commands.spawn((
        Name::new("PSX Present Camera"),
        Camera2d,
        Camera {
            order: 1,
            clear_color: ClearColorConfig::Custom(Color::BLACK),
            ..default()
        },
        RenderLayers::layer(PSX_CANVAS_LAYER),
    ));
}

fn tag_scene_descendants_with_psxify(
    mut commands: Commands,
    scene_spawner: Res<SceneSpawner>,
    q_scene: Query<(Entity, &SceneInstance), (With<PsxWorldRoot>, Without<PsxSceneTagged>)>,
    children: Query<&Children>,
) {
    for (root, instance) in &q_scene {
        if !scene_spawner.instance_is_ready(**instance) {
            continue;
        }
        mark_descendants(&mut commands, &children, root);
        commands.entity(root).insert(PsxSceneTagged);
    }
}

fn mark_descendants(commands: &mut Commands, children_q: &Query<&Children>, entity: Entity) {
    commands.entity(entity).insert(Psxify);
    if let Ok(children) = children_q.get(entity) {
        for child in children.iter() {
            mark_descendants(commands, children_q, child);
        }
    }
}

fn swap_standard_to_extended(
    mut commands: Commands,
    q: Query<(Entity, &MeshMaterial3d<StandardMaterial>), (With<Psxify>, Without<PsxConverted>)>,
    std_mats: Res<Assets<StandardMaterial>>,
    mut ext_assets: ResMut<Assets<PsxPbrMaterial>>,
    q_cam: Query<&PsxConfig, With<PsxCamera>>,
) {
    let cfg = q_cam.single().ok().copied().unwrap_or_default();
    for (entity, std_handle_comp) in &q {
        let std_handle = std_handle_comp.0.clone();
        let Some(std_mat) = std_mats.get(&std_handle) else {
            continue;
        };
        // Keep per-entity extended materials so per-object focus flags do not leak
        // to other meshes that share the same StandardMaterial.
        let ext_handle = ext_assets.add(PsxPbrMaterial {
            base: std_mat.clone(),
            extension: make_ext(cfg),
        });
        commands
            .entity(entity)
            .remove::<MeshMaterial3d<StandardMaterial>>()
            .insert((MeshMaterial3d(ext_handle), PsxConverted));
    }
}

fn apply_psx_from_camera(
    q_cam: Query<&PsxConfig, (With<PsxCamera>, Changed<PsxConfig>)>,
    mut materials: ResMut<Assets<PsxPbrMaterial>>,
) {
    let Ok(cfg) = q_cam.single() else {
        return;
    };
    let flags = flags_from(*cfg);
    let resolution = Vec2::new(cfg.resolution.x as f32, cfg.resolution.y as f32);
    for (_, mat) in materials.iter_mut() {
        mat.extension.flags = (mat.extension.flags & FX_FOCUSED) | flags;
        mat.extension.resolution = resolution;
        mat.extension.quantize_steps = cfg.quantize_steps.max(2);
        mat.extension.dither_strength = cfg.dither_strength.max(0.0);
        mat.extension.dither_scale = cfg.dither_scale.max(0.25);
        mat.extension.dither_mode = cfg.dither_mode as u32;
        mat.extension.saturation = cfg.saturation.clamp(0.0, 1.0);
    }
}

fn attach_psx_camera_to_target(
    mut commands: Commands,
    target: Res<PsxRenderTarget>,
    q_new: Query<Entity, Added<PsxCamera>>,
) {
    for entity in &q_new {
        commands
            .entity(entity)
            .insert((
                Camera {
                    order: 0,
                    clear_color: ClearColorConfig::Custom(Color::BLACK),
                    ..default()
                },
                RenderTarget::Image(target.texture.clone().into()),
                bevy::core_pipeline::tonemapping::Tonemapping::None,
            ))
            // the player camera bundle is tuned for the main HDR window target
            // the PSX pass renders to an LDR offscreen image, so i disable those extras
            .remove::<bevy::render::view::Hdr>()
            .remove::<bevy::post_process::auto_exposure::AutoExposure>();
        commands
            .entity(entity)
            .remove::<bevy::anti_alias::taa::TemporalAntiAliasing>()
            .remove::<bevy::core_pipeline::prepass::DeferredPrepass>()
            .remove::<bevy::light::ShadowFilteringMethod>();
    }
}

fn resize_render_target_from_camera(
    q_cam: Query<&PsxConfig, With<PsxCamera>>,
    target: Res<PsxRenderTarget>,
    mut images: ResMut<Assets<Image>>,
) {
    let Ok(cfg) = q_cam.single() else {
        return;
    };
    let width = cfg.resolution.x.max(1);
    let height = cfg.resolution.y.max(1);
    let Some(img) = images.get_mut(&target.texture) else {
        return;
    };
    let cur = img.texture_descriptor.size;
    if cur.width == width && cur.height == height {
        return;
    }
    img.resize(Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    });
    img.sampler = ImageSampler::nearest();
}

fn fit_canvas_to_window(
    windows: Query<&Window, With<PrimaryWindow>>,
    q_cam: Query<&PsxConfig, With<PsxCamera>>,
    mut q_canvas: Query<(&mut Transform, &mut Sprite), With<PsxLowResCanvas>>,
) {
    let Ok(win) = windows.single() else {
        return;
    };
    let Ok(cfg) = q_cam.single() else {
        return;
    };

    let (_canvas_min, _canvas_max, rw, rh, s) = canvas_metrics(win, *cfg);

    for (mut tf, mut spr) in &mut q_canvas {
        tf.translation = Vec3::ZERO;
        tf.scale = Vec3::new(s, s, 1.0);
        spr.custom_size = Some(Vec2::new(rw, rh));
    }
}

fn spawn_psx_pointer(mut commands: Commands) {
    let id = PointerId::Custom(Uuid::from_u128(PSX_POINTER_UUID));
    commands.spawn((
        Name::new("PSX Picking Pointer"),
        PsxPickingPointer,
        id,
        PointerLocation::default(),
    ));
}

fn canvas_metrics(win: &Window, cfg: PsxConfig) -> (Vec2, Vec2, f32, f32, f32) {
    let rw = cfg.resolution.x.max(1) as f32;
    let rh = cfg.resolution.y.max(1) as f32;
    let win_size = Vec2::new(
        win.resolution.width().max(1.0),
        win.resolution.height().max(1.0),
    );
    let sx = win_size.x / rw;
    let sy = win_size.y / rh;
    // use integer cover scaling: no filtering blur, no letterboxing
    let s = sx.max(sy).ceil().max(1.0);
    let half = Vec2::new(rw * s, rh * s) * 0.5;
    let center = win_size * 0.5;
    (center - half, center + half, rw, rh, s)
}

fn compute_psx_location(win: &Window, cfg: PsxConfig, target: &RenderTarget) -> Option<Location> {
    let cursor = win.cursor_position()?;
    let (canvas_min, canvas_max, _rw, _rh, s) = canvas_metrics(win, cfg);
    if cursor.x < canvas_min.x
        || cursor.x > canvas_max.x
        || cursor.y < canvas_min.y
        || cursor.y > canvas_max.y
    {
        return None;
    }

    let pix = (cursor - canvas_min) / s;
    let norm = target.normalize(None)?;
    Some(Location {
        target: norm,
        position: pix,
    })
}

fn update_psx_pointer_location(
    windows: Query<&Window, With<PrimaryWindow>>,
    q_cam: Query<(&PsxConfig, &RenderTarget), With<PsxCamera>>,
    mut q_ptr: Query<(&PointerId, &mut PointerLocation), With<PsxPickingPointer>>,
    mut inputs: MessageWriter<PointerInput>,
) {
    let Ok(win) = windows.single() else {
        return;
    };
    let Ok((cfg, target)) = q_cam.single() else {
        return;
    };
    let loc = compute_psx_location(win, *cfg, target);
    for (pid, mut ploc) in &mut q_ptr {
        let delta = match (ploc.location.as_ref(), loc.as_ref()) {
            (Some(prev), Some(cur)) => cur.position - prev.position,
            _ => Vec2::ZERO,
        };
        ploc.location = loc.clone();
        let Some(loc) = loc.as_ref() else {
            continue;
        };
        inputs.write(PointerInput {
            pointer_id: *pid,
            location: loc.clone(),
            action: PointerAction::Move { delta },
        });
    }
}

fn forward_mouse_buttons_to_pointer(
    mut ev: MessageReader<MouseButtonInput>,
    q_ptr: Query<(&PointerId, &PointerLocation), With<PsxPickingPointer>>,
    mut inputs: MessageWriter<PointerInput>,
) {
    let Ok((pid, ploc)) = q_ptr.single() else {
        return;
    };
    let Some(loc) = ploc.location.as_ref() else {
        return;
    };
    let button_to_pointer = |button: MouseButton| -> PointerButton {
        match button {
            MouseButton::Left => PointerButton::Primary,
            MouseButton::Right => PointerButton::Secondary,
            MouseButton::Middle => PointerButton::Middle,
            _ => PointerButton::Primary,
        }
    };

    for event in ev.read() {
        let btn = button_to_pointer(event.button);
        let action = match event.state {
            ButtonState::Pressed => PointerAction::Press(btn),
            ButtonState::Released => PointerAction::Release(btn),
        };
        inputs.write(PointerInput {
            pointer_id: *pid,
            location: loc.clone(),
            action,
        });
    }
}
