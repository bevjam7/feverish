use bevy::{
    prelude::*,
    text::{Justify, LineBreak, TextLayout},
    ui::FocusPolicy,
};

use super::{
    components::{
        ButtonAction, DisabledButton, DiscoveryKind, GalleryDetailDescription, GalleryDetailStatus,
        GalleryDetailSubtitle, GalleryDetailTitle, GalleryListCache, GalleryListRoot,
        MainMenuGalleryPanel, MainMenuHeading, MainMenuLine, MainMenuPage, MainMenuSettingsPanel,
        MainMenuState, MainMenuTab, MainMenuTerminalPanel, MainMenuTicker, MenuButton,
        MenuConfirmState, MenuKind, MenuOwner, MenuRoot, SettingsValueText,
    },
    confirm_popup::spawn_confirm_popup,
    systems::UiFonts,
    theme,
};
use crate::settings::SettingKey;

pub(super) fn spawn_main_menu(commands: &mut Commands, fonts: &UiFonts, owner: Entity) -> Entity {
    let root = commands
        .spawn((
            Name::new("Main Menu UI"),
            MenuRoot {
                owner,
                kind: MenuKind::Main,
            },
            MainMenuState {
                owner,
                page: MainMenuPage::Home,
                selected_item: None,
                selected_npc: None,
            },
            MenuConfirmState {
                owner,
                pending: None,
            },
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(theme::OVERLAY),
            FocusPolicy::Block,
            GlobalZIndex(90),
        ))
        .id();

    commands.entity(root).with_children(|overlay| {
        overlay
            .spawn((
                Node {
                    width: Val::Px(theme::UI_WIDTH),
                    height: Val::Px(theme::UI_HEIGHT),
                    padding: UiRect::all(Val::Px(8.0)),
                    border: UiRect::all(Val::Px(3.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(8.0),
                    ..default()
                },
                BackgroundColor(theme::PANEL_BG),
                theme::border(true),
            ))
            .with_children(|frame| {
                frame
                    .spawn((
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(48.0),
                            border: UiRect::all(Val::Px(2.0)),
                            padding: UiRect::horizontal(Val::Px(12.0)),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::SpaceBetween,
                            ..default()
                        },
                        BackgroundColor(theme::PANEL_ALT),
                        theme::border(false),
                    ))
                    .with_children(|titlebar| {
                        titlebar.spawn((
                            Text::new("FEVERISH"),
                            TextFont {
                                font: fonts.pixel.clone(),
                                font_size: 22.0,
                                ..default()
                            },
                            TextColor(theme::TEXT_DARK),
                        ));

                        titlebar.spawn((
                            Text::new("main menu"),
                            TextFont {
                                font: fonts.body.clone(),
                                font_size: 30.0,
                                ..default()
                            },
                            TextColor(theme::TEXT_DARK),
                        ));
                    });

                frame
                    .spawn(Node {
                        flex_grow: 1.0,
                        min_height: Val::Px(0.0),
                        width: Val::Percent(100.0),
                        column_gap: Val::Px(10.0),
                        ..default()
                    })
                    .with_children(|body| {
                        body.spawn((
                            Node {
                                width: Val::Px(290.0),
                                min_width: Val::Px(290.0),
                                max_width: Val::Px(290.0),
                                flex_shrink: 0.0,
                                height: Val::Percent(100.0),
                                border: UiRect::all(Val::Px(2.0)),
                                padding: UiRect::all(Val::Px(8.0)),
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(8.0),
                                ..default()
                            },
                            BackgroundColor(theme::PANEL_ALT),
                            theme::border(false),
                        ))
                        .with_children(|menu| {
                            spawn_main_button(
                                menu,
                                fonts,
                                owner,
                                "START",
                                ButtonAction::Play,
                                false,
                            );
                            spawn_main_button(
                                menu,
                                fonts,
                                owner,
                                "CONTINUE",
                                ButtonAction::Continue,
                                true,
                            );
                            spawn_main_tab_button(
                                menu,
                                fonts,
                                owner,
                                "CREDITS",
                                MainMenuPage::Credits,
                            );
                            spawn_main_tab_button(
                                menu,
                                fonts,
                                owner,
                                "DISCOVERED ITEMS",
                                MainMenuPage::DiscoveredItems,
                            );
                            spawn_main_tab_button(
                                menu,
                                fonts,
                                owner,
                                "PHONE LIST",
                                MainMenuPage::PhoneList,
                            );
                            spawn_main_tab_button(
                                menu,
                                fonts,
                                owner,
                                "SETTINGS",
                                MainMenuPage::Settings,
                            );
                            spawn_main_tab_button(menu, fonts, owner, "HOME", MainMenuPage::Home);
                            spawn_main_button(
                                menu,
                                fonts,
                                owner,
                                "QUIT",
                                ButtonAction::QuitGame,
                                false,
                            );
                        });

                        body.spawn((
                            Node {
                                flex_grow: 1.0,
                                min_width: Val::Px(0.0),
                                min_height: Val::Px(0.0),
                                height: Val::Percent(100.0),
                                border: UiRect::all(Val::Px(2.0)),
                                padding: UiRect::all(Val::Px(8.0)),
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(8.0),
                                overflow: Overflow::clip_y(),
                                ..default()
                            },
                            BackgroundColor(theme::SCREEN_BG),
                            theme::border(false),
                        ))
                        .with_children(|panel| {
                            panel.spawn((
                                Text::new("SYSTEM STATUS"),
                                MainMenuHeading { owner },
                                TextFont {
                                    font: fonts.pixel.clone(),
                                    font_size: 16.0,
                                    ..default()
                                },
                                TextColor(theme::TEXT_LIGHT),
                            ));

                            panel
                                .spawn((
                                    MainMenuTerminalPanel { owner },
                                    Node {
                                        width: Val::Percent(100.0),
                                        border: UiRect::all(Val::Px(2.0)),
                                        padding: UiRect::all(Val::Px(8.0)),
                                        flex_direction: FlexDirection::Column,
                                        row_gap: Val::Px(4.0),
                                        display: Display::Flex,
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgb(0.06, 0.08, 0.09)),
                                    theme::border(false),
                                ))
                                .with_children(|lines| {
                                    for index in 0..8 {
                                        lines.spawn((
                                            Text::new(""),
                                            MainMenuLine { owner, index },
                                            TextFont {
                                                font: fonts.body.clone(),
                                                font_size: 26.0,
                                                ..default()
                                            },
                                            TextColor(theme::CRT_GREEN),
                                        ));
                                    }
                                });

                            panel
                                .spawn((
                                    MainMenuGalleryPanel { owner },
                                    Node {
                                        width: Val::Percent(100.0),
                                        flex_grow: 1.0,
                                        border: UiRect::all(Val::Px(2.0)),
                                        padding: UiRect::all(Val::Px(8.0)),
                                        column_gap: Val::Px(8.0),
                                        display: Display::None,
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgb(0.05, 0.06, 0.07)),
                                    theme::border(false),
                                ))
                                .with_children(|gallery| {
                                    gallery
                                        .spawn((
                                            GalleryListRoot { owner },
                                            // list cache to avoid rebuilding every frame
                                            GalleryListCache {
                                                owner,
                                                kind: DiscoveryKind::Item,
                                                revision: u64::MAX,
                                                selected: None,
                                            },
                                            Node {
                                                width: Val::Px(240.0),
                                                min_width: Val::Px(240.0),
                                                max_width: Val::Px(240.0),
                                                flex_shrink: 0.0,
                                                height: Val::Percent(100.0),
                                                min_height: Val::Px(0.0),
                                                border: UiRect::all(Val::Px(2.0)),
                                                padding: UiRect::all(Val::Px(6.0)),
                                                flex_direction: FlexDirection::Column,
                                                row_gap: Val::Px(6.0),
                                                overflow: Overflow::scroll_y(),
                                                ..default()
                                            },
                                            ScrollPosition(Vec2::ZERO),
                                            Interaction::default(),
                                            BackgroundColor(Color::srgb(0.03, 0.04, 0.05)),
                                            theme::border(false),
                                        ))
                                        .with_children(|list| {
                                            list.spawn((
                                                Text::new("loading gallery"),
                                                TextFont {
                                                    font: fonts.body.clone(),
                                                    font_size: 24.0,
                                                    ..default()
                                                },
                                                TextColor(theme::CRT_GREEN),
                                            ));
                                        });

                                    gallery
                                        .spawn((
                                            Node {
                                                flex_grow: 1.0,
                                                height: Val::Percent(100.0),
                                                flex_direction: FlexDirection::Column,
                                                row_gap: Val::Px(6.0),
                                                ..default()
                                            },
                                            BackgroundColor(Color::NONE),
                                        ))
                                        .with_children(|detail| {
                                            detail.spawn((
                                                GalleryDetailTitle { owner },
                                                Text::new("--"),
                                                TextFont {
                                                    font: fonts.pixel.clone(),
                                                    font_size: 12.0,
                                                    ..default()
                                                },
                                                TextColor(theme::TEXT_LIGHT),
                                            ));

                                            detail.spawn((
                                                GalleryDetailSubtitle { owner },
                                                Text::new(""),
                                                TextFont {
                                                    font: fonts.body.clone(),
                                                    font_size: 25.0,
                                                    ..default()
                                                },
                                                TextColor(theme::CRT_GREEN),
                                            ));

                                            detail.spawn((
                                                GalleryDetailStatus { owner },
                                                Text::new(""),
                                                TextFont {
                                                    font: fonts.body.clone(),
                                                    font_size: 22.0,
                                                    ..default()
                                                },
                                                TextColor(theme::TEXT_LIGHT),
                                            ));

                                            detail
                                                .spawn((
                                                    Node {
                                                        width: Val::Percent(100.0),
                                                        flex_grow: 1.0,
                                                        min_height: Val::Px(0.0),
                                                        border: UiRect::all(Val::Px(2.0)),
                                                        padding: UiRect::all(Val::Px(8.0)),
                                                        overflow: Overflow::scroll_y(),
                                                        ..default()
                                                    },
                                                    ScrollPosition(Vec2::ZERO),
                                                    Interaction::default(),
                                                    BackgroundColor(Color::srgb(0.02, 0.03, 0.04)),
                                                    theme::border(false),
                                                ))
                                                .with_children(|text_panel| {
                                                    text_panel.spawn((
                                                        GalleryDetailDescription { owner },
                                                        Text::new(
                                                            "select an entry to inspect details",
                                                        ),
                                                        TextFont {
                                                            font: fonts.body.clone(),
                                                            font_size: 23.0,
                                                            ..default()
                                                        },
                                                        TextColor(theme::CRT_GREEN),
                                                    ));
                                                });
                                        });
                                });

                            panel
                                .spawn((
                                    MainMenuSettingsPanel { owner },
                                    Node {
                                        width: Val::Percent(100.0),
                                        flex_grow: 1.0,
                                        min_height: Val::Px(0.0),
                                        border: UiRect::all(Val::Px(2.0)),
                                        padding: UiRect::all(Val::Px(8.0)),
                                        row_gap: Val::Px(6.0),
                                        flex_direction: FlexDirection::Column,
                                        overflow: Overflow::clip_y(),
                                        display: Display::None,
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgb(0.04, 0.05, 0.06)),
                                    theme::border(false),
                                ))
                                .with_children(|settings_panel| {
                                    settings_panel.spawn((
                                        Text::new("SETTINGS"),
                                        TextFont {
                                            font: fonts.pixel.clone(),
                                            font_size: 12.0,
                                            ..default()
                                        },
                                        TextColor(theme::TEXT_LIGHT),
                                    ));

                                    settings_panel
                                        .spawn((
                                            Node {
                                                width: Val::Percent(100.0),
                                                height: Val::Percent(100.0),
                                                flex_grow: 1.0,
                                                min_height: Val::Px(0.0),
                                                flex_direction: FlexDirection::Column,
                                                row_gap: Val::Px(6.0),
                                                overflow: Overflow::scroll_y(),
                                                padding: UiRect::right(Val::Px(4.0)),
                                                ..default()
                                            },
                                            ScrollPosition(Vec2::ZERO),
                                            Interaction::default(),
                                        ))
                                        .with_children(|list| {
                                            for key in SettingKey::ALL {
                                                spawn_settings_row(list, fonts, owner, key);
                                            }
                                        });
                                });

                            panel
                                .spawn((
                                    Node {
                                        width: Val::Percent(100.0),
                                        height: Val::Px(52.0),
                                        min_height: Val::Px(52.0),
                                        max_height: Val::Px(52.0),
                                        border: UiRect::all(Val::Px(2.0)),
                                        padding: UiRect::all(Val::Px(6.0)),
                                        overflow: Overflow::clip_x(),
                                        align_items: AlignItems::Center,
                                        justify_content: JustifyContent::Center,
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgb(0.02, 0.03, 0.04)),
                                    theme::border(false),
                                ))
                                .with_children(|ticker| {
                                    ticker.spawn((
                                        MainMenuTicker {
                                            tips: vec![
                                                "there should be a super useful tip here"
                                                    .to_string(),
                                            ],
                                            current: 0,
                                            offset_x: 1280.0,
                                            pause_timer: 0.0,
                                        },
                                        Node {
                                            position_type: PositionType::Absolute,
                                            left: Val::Px(0.0),
                                            top: Val::Px(2.0),
                                            ..default()
                                        },
                                        Text::new("there should be a super useful tip here"),
                                        TextFont {
                                            font: fonts.body.clone(),
                                            font_size: 24.0,
                                            ..default()
                                        },
                                        TextColor(theme::CRT_GREEN),
                                        TextLayout::new(Justify::Left, LineBreak::NoWrap),
                                        UiTransform::from_translation(Val2::px(1280.0, 0.0)),
                                    ));
                                });
                        });
                    });

                frame
                    .spawn((
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(44.0),
                            border: UiRect::all(Val::Px(2.0)),
                            padding: UiRect::horizontal(Val::Px(10.0)),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::SpaceBetween,
                            ..default()
                        },
                        BackgroundColor(theme::PANEL_ALT),
                        theme::border(false),
                    ))
                    .with_children(|status| {
                        status
                            .spawn(Node {
                                flex_grow: 1.0,
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            })
                            .with_children(|label| {
                                label.spawn((
                                    Text::new("Cool Company Inc"),
                                    TextFont {
                                        font: fonts.pixel.clone(),
                                        font_size: 14.0,
                                        ..default()
                                    },
                                    TextColor(theme::TEXT_DARK),
                                    TextLayout::new(Justify::Left, LineBreak::NoWrap),
                                ));
                            });
                    });

                spawn_confirm_popup(frame, fonts, owner);
            });
    });

    root
}

fn spawn_main_button(
    parent: &mut ChildSpawnerCommands,
    fonts: &UiFonts,
    owner: Entity,
    label: &str,
    action: ButtonAction,
    disabled: bool,
) {
    let mut button = parent.spawn((
        Button,
        MenuOwner(owner),
        MenuButton {
            action,
            raised: true,
        },
        Node {
            width: Val::Percent(100.0),
            min_height: Val::Px(38.0),
            border: UiRect::all(Val::Px(2.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(if disabled {
            theme::BUTTON_DISABLED
        } else {
            theme::BUTTON_BG
        }),
        theme::border(true),
    ));

    if disabled {
        button.insert(DisabledButton);
    }

    button.with_children(|b| {
        b.spawn((
            Text::new(label),
            TextFont {
                font: fonts.pixel.clone(),
                font_size: 13.0,
                ..default()
            },
            TextColor(theme::TEXT_DARK),
        ));
    });
}

fn spawn_main_tab_button(
    parent: &mut ChildSpawnerCommands,
    fonts: &UiFonts,
    owner: Entity,
    label: &str,
    page: MainMenuPage,
) {
    parent
        .spawn((
            Button,
            MenuOwner(owner),
            MainMenuTab { owner, page },
            MenuButton {
                action: ButtonAction::SelectPage(page),
                raised: true,
            },
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(34.0),
                border: UiRect::all(Val::Px(2.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(theme::BUTTON_BG),
            theme::border(true),
        ))
        .with_children(|b| {
            b.spawn((
                Text::new(label),
                TextFont {
                    font: fonts.pixel.clone(),
                    font_size: 10.0,
                    ..default()
                },
                TextColor(theme::TEXT_DARK),
            ));
        });
}

fn spawn_settings_row(
    parent: &mut ChildSpawnerCommands,
    fonts: &UiFonts,
    owner: Entity,
    key: SettingKey,
) {
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(36.0),
                border: UiRect::all(Val::Px(2.0)),
                padding: UiRect::horizontal(Val::Px(6.0)),
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.08, 0.09, 0.11)),
            theme::border(true),
        ))
        .with_children(|row| {
            row.spawn(Node {
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                ..default()
            })
            .with_children(|label| {
                label.spawn((
                    Text::new(key.label()),
                    TextFont {
                        font: fonts.pixel.clone(),
                        font_size: 10.0,
                        ..default()
                    },
                    TextColor(theme::TEXT_LIGHT),
                ));
            });

            row.spawn(Node {
                width: Val::Px(170.0),
                min_width: Val::Px(170.0),
                max_width: Val::Px(170.0),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|controls| {
                spawn_settings_step_button(controls, fonts, owner, key, "-", -1);

                controls
                    .spawn(Node {
                        width: Val::Px(82.0),
                        min_width: Val::Px(82.0),
                        max_width: Val::Px(82.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|value_box| {
                        value_box.spawn((
                            SettingsValueText { owner, key },
                            Text::new("--"),
                            TextFont {
                                font: fonts.body.clone(),
                                font_size: 24.0,
                                ..default()
                            },
                            TextColor(theme::CRT_GREEN),
                        ));
                    });

                spawn_settings_step_button(controls, fonts, owner, key, "+", 1);
            });
        });
}

fn spawn_settings_step_button(
    parent: &mut ChildSpawnerCommands,
    fonts: &UiFonts,
    owner: Entity,
    key: SettingKey,
    label: &str,
    step: i32,
) {
    parent
        .spawn((
            Button,
            MenuOwner(owner),
            MenuButton {
                action: ButtonAction::AdjustSetting(key, step),
                raised: true,
            },
            Node {
                width: Val::Px(30.0),
                min_width: Val::Px(30.0),
                max_width: Val::Px(30.0),
                min_height: Val::Px(24.0),
                border: UiRect::all(Val::Px(2.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(theme::BUTTON_BG),
            theme::border(true),
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(label),
                TextFont {
                    font: fonts.pixel.clone(),
                    font_size: 11.0,
                    ..default()
                },
                TextColor(theme::TEXT_DARK),
            ));
        });
}
