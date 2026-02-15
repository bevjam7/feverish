use bevy::{
    camera::{RenderTarget, visibility::RenderLayers},
    image::ImageSampler,
    prelude::*,
    reflect::TypePath,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
    sprite_render::{AlphaMode2d, Material2d, Material2dPlugin},
    ui::IsDefaultUiCamera,
    window::PrimaryWindow,
};

use super::{DialogueUiRoot, InventoryUiRoot, MainMenuUi, PauseMenuUi, hint::UiHintRoot};
use crate::settings::GameSettings;

const UI_FX_SHADER: &str = "shaders/ui_menu_fx.wgsl";
const UI_FX_LAYER: usize = 31;
const UI_FX_PIXEL_SIZE: f32 = 1.35;
const UI_FX_QUANT_STEPS: f32 = 96.0;
const UI_FX_DITHER_STRENGTH: f32 = 0.20;
const UI_FX_MELT_STRENGTH: f32 = 0.14;

pub(super) struct UiMenuFxPlugin;

impl Plugin for UiMenuFxPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(Material2dPlugin::<UiMenuFxMaterial>::default())
            .add_systems(
                Startup,
                (
                    setup_ui_fx_target_and_material,
                    spawn_ui_fx_present_camera,
                    spawn_ui_fx_quad,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (
                    bind_default_ui_camera_to_fx_target,
                    resize_ui_fx_target,
                    fit_ui_fx_quad_to_window,
                    update_ui_fx_material,
                ),
            );
    }
}

#[derive(Resource)]
struct UiFxTarget {
    texture: Handle<Image>,
}

#[derive(Resource)]
struct UiFxMaterialHandle {
    handle: Handle<UiMenuFxMaterial>,
}

#[derive(Component)]
struct UiFxQuad;

#[derive(Component)]
struct UiFxPresentCamera;

#[derive(Component)]
struct UiFxBoundDefaultCamera;

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
struct UiMenuFxMaterial {
    #[texture(0)]
    #[sampler(1)]
    source: Handle<Image>,
    // rgb tint + intensity in a
    #[uniform(2)]
    tint: Vec4,
    // x: pixel_size, y: quant_steps, z: dither_strength, w: liquid_strength
    #[uniform(3)]
    params_a: Vec4,
    // x: glitch_chance, y: glitch_speed, z: monitor_distortion_on, w: effect_mix
    #[uniform(4)]
    params_b: Vec4,
    // x: width, y: height, z: inv_width, w: inv_height
    #[uniform(5)]
    viewport: Vec4,
    // x: cursor_u, y: cursor_v, z: cursor_visible, w: cursor_distortion_on
    #[uniform(6)]
    cursor: Vec4,
}

impl Material2d for UiMenuFxMaterial {
    fn fragment_shader() -> ShaderRef {
        UI_FX_SHADER.into()
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

fn setup_ui_fx_target_and_material(
    mut commands: Commands,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<UiMenuFxMaterial>>,
) {
    let (width, height) = windows
        .single()
        .ok()
        .map(|w| {
            (
                w.resolution.width().max(1.0) as u32,
                w.resolution.height().max(1.0) as u32,
            )
        })
        .unwrap_or((1280, 720));

    let mut target = Image::new_target_texture(
        width,
        height,
        bevy::render::render_resource::TextureFormat::Bgra8UnormSrgb,
        None,
    );
    target.sampler = ImageSampler::linear();
    let target_handle = images.add(target);

    let material_handle = materials.add(UiMenuFxMaterial {
        source: target_handle.clone(),
        tint: Vec4::new(0.95, 0.92, 1.0, 0.05),
        params_a: Vec4::new(
            UI_FX_PIXEL_SIZE,
            UI_FX_QUANT_STEPS,
            UI_FX_DITHER_STRENGTH,
            UI_FX_MELT_STRENGTH,
        ),
        params_b: Vec4::new(0.30, 0.55, 0.0, 0.0),
        viewport: Vec4::new(
            width as f32,
            height as f32,
            1.0 / width.max(1) as f32,
            1.0 / height.max(1) as f32,
        ),
        cursor: Vec4::ZERO,
    });

    commands.insert_resource(UiFxTarget {
        texture: target_handle,
    });
    commands.insert_resource(UiFxMaterialHandle {
        handle: material_handle,
    });
}

fn spawn_ui_fx_present_camera(mut commands: Commands) {
    commands.spawn((
        Name::new("UI FX Present Camera"),
        Camera2d,
        Camera {
            order: 3,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        RenderLayers::layer(UI_FX_LAYER),
        UiFxPresentCamera,
    ));
}

fn bind_default_ui_camera_to_fx_target(
    mut commands: Commands,
    target: Res<UiFxTarget>,
    mut q_ui_camera: Query<
        (Entity, &mut Camera),
        (With<IsDefaultUiCamera>, Without<UiFxBoundDefaultCamera>),
    >,
) {
    for (entity, mut camera) in &mut q_ui_camera {
        camera.clear_color = ClearColorConfig::Custom(Color::NONE);
        commands.entity(entity).insert((
            RenderTarget::Image(target.texture.clone().into()),
            UiFxBoundDefaultCamera,
        ));
    }
}

fn spawn_ui_fx_quad(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    material: Res<UiFxMaterialHandle>,
) {
    commands.spawn((
        Name::new("UI FX Quad"),
        Mesh2d(meshes.add(Rectangle::default())),
        MeshMaterial2d(material.handle.clone()),
        Transform::from_xyz(0.0, 0.0, 0.0),
        RenderLayers::layer(UI_FX_LAYER),
        Pickable::IGNORE,
        UiFxQuad,
    ));
}

fn resize_ui_fx_target(
    windows: Query<&Window, With<PrimaryWindow>>,
    target: Res<UiFxTarget>,
    mut images: ResMut<Assets<Image>>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let width = window.resolution.width().max(1.0) as u32;
    let height = window.resolution.height().max(1.0) as u32;

    let Some(image) = images.get_mut(&target.texture) else {
        return;
    };
    if image.texture_descriptor.size.width == width
        && image.texture_descriptor.size.height == height
    {
        return;
    }

    image.resize(bevy::render::render_resource::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    });
    image.sampler = ImageSampler::linear();
}

fn fit_ui_fx_quad_to_window(
    windows: Query<&Window, With<PrimaryWindow>>,
    mut q_quad: Query<&mut Transform, With<UiFxQuad>>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let width = window.resolution.width().max(1.0);
    let height = window.resolution.height().max(1.0);

    for mut transform in &mut q_quad {
        transform.translation = Vec3::ZERO;
        transform.scale = Vec3::new(width, height, 1.0);
    }
}

fn update_ui_fx_material(
    windows: Query<&Window, With<PrimaryWindow>>,
    menus: Query<
        (),
        Or<(
            With<MainMenuUi>,
            With<PauseMenuUi>,
            With<DialogueUiRoot>,
            With<InventoryUiRoot>,
            With<UiHintRoot>,
        )>,
    >,
    settings: Res<GameSettings>,
    time: Res<Time>,
    material_handle: Res<UiFxMaterialHandle>,
    mut materials: ResMut<Assets<UiMenuFxMaterial>>,
    mut quad_visibility: Query<&mut Visibility, With<UiFxQuad>>,
    mut present_cameras: Query<&mut Camera, With<UiFxPresentCamera>>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let Some(material) = materials.get_mut(&material_handle.handle) else {
        return;
    };

    let width = window.resolution.width().max(1.0);
    let height = window.resolution.height().max(1.0);
    material.viewport = Vec4::new(width, height, 1.0 / width, 1.0 / height);

    let menu_visible = !menus.is_empty();
    for mut visibility in &mut quad_visibility {
        *visibility = if menu_visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    for mut camera in &mut present_cameras {
        camera.is_active = menu_visible;
    }

    let shader_on = menu_visible && settings.ui_fx;
    material.params_b.w = if shader_on { 1.0 } else { 0.0 };
    material.params_b.z = if settings.ui_monitor_distortion {
        1.0
    } else {
        0.0
    };
    // keep the melt subtle and slowly breathing to avoid a constant wacky look
    let pulse = (time.elapsed_secs() * 0.23).sin() * 0.5 + 0.5;
    material.params_a.w = 0.14 + pulse * 0.08;

    material.cursor = if menu_visible {
        window
            .cursor_position()
            .map(|pos| {
                Vec4::new(
                    pos.x / width,
                    pos.y / height,
                    1.0,
                    if settings.ui_cursor_distortion {
                        1.0
                    } else {
                        0.0
                    },
                )
            })
            .unwrap_or(Vec4::ZERO)
    } else {
        Vec4::ZERO
    };
}
