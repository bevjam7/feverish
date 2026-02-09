use bevy::{prelude::*, text::TextLayout, ui::FocusPolicy};

use super::{
    components::{ButtonAction, ConfirmDialogMessage, ConfirmDialogRoot, MenuButton, MenuOwner},
    systems::UiFonts,
    theme,
};

pub(super) fn spawn_confirm_popup(
    parent: &mut ChildSpawnerCommands,
    fonts: &UiFonts,
    owner: Entity,
) {
    parent
        .spawn((
            ConfirmDialogRoot { owner },
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            FocusPolicy::Block,
            GlobalZIndex(150),
        ))
        .with_children(|overlay| {
            overlay
                .spawn((
                    Node {
                        width: Val::Px(480.0),
                        border: UiRect::all(Val::Px(3.0)),
                        padding: UiRect::all(Val::Px(10.0)),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(10.0),
                        ..default()
                    },
                    BackgroundColor(theme::PANEL_BG),
                    theme::border(true),
                ))
                .with_children(|panel| {
                    panel.spawn((
                        ConfirmDialogMessage { owner },
                        // invincible???
                        Text::new("are you sure?"),
                        TextFont {
                            font: fonts.pixel.clone(),
                            font_size: 12.0,
                            ..default()
                        },
                        TextColor(theme::TEXT_DARK),
                        TextLayout::new(Justify::Left, LineBreak::WordBoundary),
                    ));

                    panel
                        .spawn(Node {
                            width: Val::Percent(100.0),
                            column_gap: Val::Px(8.0),
                            ..default()
                        })
                        .with_children(|buttons| {
                            spawn_confirm_button(
                                buttons,
                                fonts,
                                owner,
                                "NO",
                                ButtonAction::ConfirmCancel,
                            );
                            spawn_confirm_button(
                                buttons,
                                fonts,
                                owner,
                                "YES",
                                ButtonAction::ConfirmProceed,
                            );
                        });
                });
        });
}

fn spawn_confirm_button(
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
                flex_grow: 1.0,
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
                    font_size: 11.0,
                    ..default()
                },
                TextColor(theme::TEXT_DARK),
            ));
        });
}
