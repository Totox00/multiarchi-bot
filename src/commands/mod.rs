pub mod cancel_preclaims;
pub mod claim;
pub mod claimed;
pub mod done;
pub mod finish_world;
pub mod get_preclaims;
pub mod mark_free;
pub mod new_world;
pub mod public;
pub mod reschedule_preclaims;
pub mod status;
pub mod status_report;
pub mod track_world;
pub mod unclaim;
pub mod unclaimed;
pub mod view_preclaims;
pub mod worlds;

use crate::{
    autocomplete::Autocomplete,
    commands::{
        cancel_preclaims::CancelPreclaimsCommand, claim::ClaimCommand, claimed::ClaimedCommand, done::DoneCommand, finish_world::FinishWorldCommand, get_preclaims::GetPreclaimsCommand,
        mark_free::MarkFreeCommand, new_world::NewWorldCommand, public::PublicCommand, reschedule_preclaims::ReschedulePreclaimsCommand, status::StatusCommand, status_report::StatusReportCommand,
        track_world::TrackWorldCommand, unclaim::UnclaimCommand, unclaimed::UnclaimedCommand, view_preclaims::ViewPreclaimsCommand, worlds::WorldsCommand,
    },
};
use serenity::all::{Command as SerenityCommand, CommandInteraction, Context, CreateCommand, Interaction};

use crate::Bot;

pub async fn register_all(ctx: &Context) {
    register::<ViewPreclaimsCommand>(ctx).await;
    register::<NewWorldCommand>(ctx).await;
    register::<GetPreclaimsCommand>(ctx).await;
    register::<TrackWorldCommand>(ctx).await;
    register::<ClaimCommand>(ctx).await;
    register::<StatusCommand>(ctx).await;
    register::<StatusReportCommand>(ctx).await;
    register::<UnclaimCommand>(ctx).await;
    register::<MarkFreeCommand>(ctx).await;
    register::<PublicCommand>(ctx).await;
    register::<UnclaimedCommand>(ctx).await;
    register::<ClaimedCommand>(ctx).await;
    register::<FinishWorldCommand>(ctx).await;
    register::<ReschedulePreclaimsCommand>(ctx).await;
    register::<CancelPreclaimsCommand>(ctx).await;
    register::<WorldsCommand>(ctx).await;
    register::<DoneCommand>(ctx).await;
}

pub async fn interaction_create(bot: &Bot, ctx: Context, interaction: Interaction) {
    match interaction {
        Interaction::Command(command) => match command.data.name.as_str() {
            ViewPreclaimsCommand::NAME => ViewPreclaimsCommand::execute(bot, ctx, command).await,
            NewWorldCommand::NAME => NewWorldCommand::execute(bot, ctx, command).await,
            GetPreclaimsCommand::NAME => GetPreclaimsCommand::execute(bot, ctx, command).await,
            TrackWorldCommand::NAME => TrackWorldCommand::execute(bot, ctx, command).await,
            ClaimCommand::NAME => ClaimCommand::execute(bot, ctx, command).await,
            StatusCommand::NAME => StatusCommand::execute(bot, ctx, command).await,
            StatusReportCommand::NAME => StatusReportCommand::execute(bot, ctx, command).await,
            UnclaimCommand::NAME => UnclaimCommand::execute(bot, ctx, command).await,
            MarkFreeCommand::NAME => MarkFreeCommand::execute(bot, ctx, command).await,
            PublicCommand::NAME => PublicCommand::execute(bot, ctx, command).await,
            UnclaimedCommand::NAME => UnclaimedCommand::execute(bot, ctx, command).await,
            ClaimedCommand::NAME => ClaimedCommand::execute(bot, ctx, command).await,
            FinishWorldCommand::NAME => FinishWorldCommand::execute(bot, ctx, command).await,
            ReschedulePreclaimsCommand::NAME => ReschedulePreclaimsCommand::execute(bot, ctx, command).await,
            CancelPreclaimsCommand::NAME => CancelPreclaimsCommand::execute(bot, ctx, command).await,
            WorldsCommand::NAME => WorldsCommand::execute(bot, ctx, command).await,
            DoneCommand::NAME => DoneCommand::execute(bot, ctx, command).await,
            _ => (),
        },
        Interaction::Component(component) => {
            if let Some((_, rest)) = component.data.custom_id.split_once("view-preclaims-") {
                ViewPreclaimsCommand::handle_interraction(bot, ctx, &component, rest).await;
            } else if let Some((_, rest)) = component.data.custom_id.split_once("unclaimed-") {
                UnclaimedCommand::handle_interraction(bot, ctx, &component, rest).await;
            }
        }
        Interaction::Autocomplete(interaction) => match interaction.data.name.as_str() {
            ViewPreclaimsCommand::NAME => ViewPreclaimsCommand::autocomplete(bot, ctx, interaction).await,
            NewWorldCommand::NAME => NewWorldCommand::autocomplete(bot, ctx, interaction).await,
            GetPreclaimsCommand::NAME => GetPreclaimsCommand::autocomplete(bot, ctx, interaction).await,
            TrackWorldCommand::NAME => TrackWorldCommand::autocomplete(bot, ctx, interaction).await,
            ClaimCommand::NAME => ClaimCommand::autocomplete(bot, ctx, interaction).await,
            StatusCommand::NAME => StatusCommand::autocomplete(bot, ctx, interaction).await,
            StatusReportCommand::NAME => StatusReportCommand::autocomplete(bot, ctx, interaction).await,
            UnclaimCommand::NAME => UnclaimCommand::autocomplete(bot, ctx, interaction).await,
            MarkFreeCommand::NAME => MarkFreeCommand::autocomplete(bot, ctx, interaction).await,
            PublicCommand::NAME => PublicCommand::autocomplete(bot, ctx, interaction).await,
            UnclaimedCommand::NAME => UnclaimedCommand::autocomplete(bot, ctx, interaction).await,
            ClaimedCommand::NAME => ClaimedCommand::autocomplete(bot, ctx, interaction).await,
            FinishWorldCommand::NAME => FinishWorldCommand::autocomplete(bot, ctx, interaction).await,
            ReschedulePreclaimsCommand::NAME => ReschedulePreclaimsCommand::autocomplete(bot, ctx, interaction).await,
            CancelPreclaimsCommand::NAME => CancelPreclaimsCommand::autocomplete(bot, ctx, interaction).await,
            WorldsCommand::NAME => WorldsCommand::autocomplete(bot, ctx, interaction).await,
            DoneCommand::NAME => DoneCommand::autocomplete(bot, ctx, interaction).await,
            _ => (),
        },
        _ => (),
    }
}

async fn register<T: Command>(ctx: &Context) {
    if let Err(err) = SerenityCommand::create_global_command(&ctx.http, T::register()).await {
        println!("Failed to create {} command: {err}", T::NAME);
    }
}

trait Command {
    const NAME: &'static str;

    fn register() -> CreateCommand;
    async fn execute(bot: &Bot, ctx: Context, command: CommandInteraction);
    async fn autocomplete(_bot: &Bot, ctx: Context, command: CommandInteraction) {
        println!("Request for autocomplete for command {}", Self::NAME);
        command.no_autocomplete(&ctx).await;
    }
}
