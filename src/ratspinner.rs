//! minimal yarn-like dialogue runtime with asset-based .rat/.ron loading
//!
//! quick start:
//! `commands.write_message(ratspinner::RatCommand::Start(ratspinner::RatStart::new("npc.default").target(npc)));`
//!
//! code-first script with chainable hooks/options:
//! ```no_run
//! # use feverish::ratspinner::{RatScriptBuilder, RatNodeBuilder, RatOptionBuilder, RatCommand};
//! # let mut commands = todo!();
//! commands.write_message(RatCommand::Register(
//!     RatScriptBuilder::new("demo")
//!         .entry("start")
//!         .node(
//!             RatNodeBuilder::new("start")
//!                 .speaker("mr. d.")
//!                 .text("choose.")
//!                 .hook("dialog.start")
//!                 .option(RatOptionBuilder::new("left").goto("left").hook("dialog.left"))
//!                 .option(RatOptionBuilder::new("right").goto("right").hook("dialog.right")),
//!         )
//!         .node(RatNodeBuilder::new("left").speaker("mr. d.").text("left chosen."))
//!         .node(RatNodeBuilder::new("right").speaker("mr. d.").text("right chosen."))
//!         .build(),
//! ));
//! i should have used yarnspinner
//! i probably have mental issues
//! ```

mod runtime;
mod types;

use bevy::prelude::*;
use crate::AppState;
pub use runtime::RatDialogueState;
#[allow(unused_imports)]
pub use types::{
    RatCommand, RatCommandsExt, RatHookTriggered, RatNodeBuilder, RatOptionBuilder, RatScript,
    RatScriptAsset, RatScriptBuilder, RatStart,
};

pub struct RatSpinnerPlugin;

impl Plugin for RatSpinnerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<runtime::RatLibrary>()
            .init_resource::<runtime::RatRuntime>()
            .init_resource::<runtime::RatDialogueState>()
            .init_asset::<types::RatScriptAsset>()
            .init_asset_loader::<runtime::RatScriptAssetLoader>()
            .add_message::<RatCommand>()
            .add_message::<RatHookTriggered>()
            .add_systems(
                OnEnter(AppState::Game),
                (
                    runtime::load_scripts_from_assets,
                    runtime::seed_builtin_script.after(runtime::load_scripts_from_assets),
                ),
            )
            .add_systems(Update, runtime::handle_rat_commands);
    }
}
