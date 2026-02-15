use bevy::{prelude::*, text::TextLayout, ui::FocusPolicy};

use super::{
    components::{
        ButtonAction, MainMenuPage, MainMenuTicker, MenuButton, MenuConfirmState, MenuKind,
        MenuOwner, MenuRoot, PauseMenuPage, PauseMenuState, PauseMenuStatusPanel,
    },
    confirm_popup::spawn_confirm_popup,
    systems::UiFonts,
    theme,
};

pub(super) fn spawn_pause_menu(commands: &mut Commands, fonts: &UiFonts, owner: Entity) -> Entity {
    let root = commands
        .spawn((
            Name::new("Pause Menu UI"),
            MenuRoot {
                owner,
                kind: MenuKind::Pause,
            },
            PauseMenuState {
                owner,
                page: PauseMenuPage::Status,
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
            GlobalZIndex(95),
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
                            Text::new("These Four"),
                            TextFont {
                                font: fonts.pixel.clone(),
                                font_size: 22.0,
                                ..default()
                            },
                            TextColor(theme::TEXT_DARK),
                        ));

                        titlebar.spawn((
                            Text::new("paused"),
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
                            spawn_pause_button(menu, fonts, owner, "RESUME", ButtonAction::Resume);
                            spawn_pause_button(
                                menu,
                                fonts,
                                owner,
                                "INVENTORY",
                                ButtonAction::OpenInventory,
                            );
                            spawn_pause_button(
                                menu,
                                fonts,
                                owner,
                                "STATUS",
                                ButtonAction::SelectPage(MainMenuPage::Home),
                            );
                            spawn_pause_button(
                                menu,
                                fonts,
                                owner,
                                "MAIN MENU",
                                ButtonAction::BackToMainMenu,
                            );
                            spawn_pause_button(menu, fonts, owner, "QUIT", ButtonAction::QuitGame);
                        });

                        body.spawn((
                            Node {
                                flex_grow: 1.0,
                                min_width: Val::Px(0.0),
                                height: Val::Percent(100.0),
                                border: UiRect::all(Val::Px(2.0)),
                                padding: UiRect::all(Val::Px(8.0)),
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(8.0),
                                ..default()
                            },
                            BackgroundColor(theme::SCREEN_BG),
                            theme::border(false),
                        ))
                        .with_children(|panel| {
                            panel
                                .spawn((
                                    PauseMenuStatusPanel { owner },
                                    Node {
                                        width: Val::Percent(100.0),
                                        flex_grow: 1.0,
                                        flex_direction: FlexDirection::Column,
                                        row_gap: Val::Px(8.0),
                                        display: Display::Flex,
                                        ..default()
                                    },
                                    BackgroundColor(Color::NONE),
                                ))
                                .with_children(|status_panel| {
                                    status_panel.spawn((
                                        Text::new("PAUSE STATUS"),
                                        TextFont {
                                            font: fonts.pixel.clone(),
                                            font_size: 16.0,
                                            ..default()
                                        },
                                        TextColor(theme::TEXT_LIGHT),
                                    ));

                                    status_panel
                                        .spawn((
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
                                            for line in [
                                                "keep up the good work!",
                                                "",
                                                "resume to continue",
                                                "main menu to reset run",
                                                "",
                                                "quit to desktop",
                                            ] {
                                                lines.spawn((
                                                    Text::new(line),
                                                    TextFont {
                                                        font: fonts.body.clone(),
                                                        font_size: 26.0,
                                                        ..default()
                                                    },
                                                    TextColor(theme::CRT_GREEN),
                                                ));
                                            }
                                        });

                                    status_panel
                                        .spawn((
                                            Node {
                                                width: Val::Percent(100.0),
                                                height: Val::Px(44.0),
                                                min_height: Val::Px(44.0),
                                                max_height: Val::Px(44.0),
                                                border: UiRect::all(Val::Px(2.0)),
                                                padding: UiRect::all(Val::Px(6.0)),
                                                overflow: Overflow::clip_x(),
                                                align_items: AlignItems::Center,
                                                justify_content: JustifyContent::Center,
                                                ..default()
                                            },
                                            BackgroundColor(Color::srgb(0.01, 0.02, 0.03)),
                                            theme::border(false),
                                        ))
                                        .with_children(|ticker| {
                                            ticker.spawn((
                                                MainMenuTicker {
                                                    tips: vec![
                                                        "press tab to open your inventory"
                                                            .to_string(),
                                                        "look at the sky!".to_string(),
                                                        "there's a hidden combo with the arrow \
                                                         keys"
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
                                                Text::new("press tab to open your inventory"),
                                                TextFont {
                                                    font: fonts.body.clone(),
                                                    font_size: 24.0,
                                                    ..default()
                                                },
                                                TextColor(theme::CRT_GREEN),
                                                TextLayout::new(Justify::Left, LineBreak::NoWrap),
                                                UiTransform::from_translation(Val2::px(
                                                    1280.0, 0.0,
                                                )),
                                            ));
                                        });
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
                            justify_content: JustifyContent::Center,
                            ..default()
                        },
                        BackgroundColor(theme::PANEL_ALT),
                        theme::border(false),
                    ))
                    .with_children(|status| {
                        status.spawn((
                            Text::new("ESC TO RESUME"),
                            TextFont {
                                font: fonts.pixel.clone(),
                                font_size: 14.0,
                                ..default()
                            },
                            TextColor(theme::TEXT_DARK),
                            TextLayout::new(Justify::Left, LineBreak::NoWrap),
                        ));
                    });

                spawn_confirm_popup(frame, fonts, owner);
            });
    });

    root
}

fn spawn_pause_button(
    parent: &mut ChildSpawnerCommands,
    fonts: &UiFonts,
    owner: Entity,
    label: &str,
    action: ButtonAction,
) {
    parent
        .spawn((
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
            BackgroundColor(theme::BUTTON_BG),
            theme::border(true),
        ))
        .with_children(|b| {
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
