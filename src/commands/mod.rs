pub mod bulk_status;
pub mod cancel_preclaims;
pub mod claim;
pub mod claimed;
pub mod done;
pub mod find;
pub mod finish_world;
pub mod get_preclaims;
pub mod mark_free;
pub mod new_reality;
pub mod new_world;
pub mod public;
pub mod register_commands;
pub mod reschedule_preclaims;
pub mod status;
pub mod status_report;
pub mod track_world;
pub mod unclaim;
pub mod unclaimed;
pub mod unpreclaim;
pub mod view_preclaims;
pub mod worlds;

use crate::{
    autocomplete::Autocomplete,
    commands::{
        bulk_status::BulkStatusCommand, cancel_preclaims::CancelPreclaimsCommand, claim::ClaimCommand, claimed::ClaimedCommand, done::DoneCommand, find::FindCommand, finish_world::FinishWorldCommand,
        get_preclaims::GetPreclaimsCommand, mark_free::MarkFreeCommand, new_reality::NewRealityCommand, new_world::NewWorldCommand, public::PublicCommand, register_commands::RegisterCommandsCommand,
        reschedule_preclaims::ReschedulePreclaimsCommand, status::StatusCommand, status_report::StatusReportCommand, track_world::TrackWorldCommand, unclaim::UnclaimCommand,
        unclaimed::UnclaimedCommand, unpreclaim::UnpreclaimCommand, view_preclaims::ViewPreclaimsCommand, worlds::WorldsCommand,
    },
};
use serenity::all::{Command as SerenityCommand, CommandInteraction, Context, CreateCommand, Interaction};

use crate::Bot;

pub async fn register_all(ctx: &Context) -> Result<Vec<SerenityCommand>, serenity::Error> {
    SerenityCommand::set_global_commands(
        &ctx.http,
        vec![
            ViewPreclaimsCommand::register(),
            NewWorldCommand::register(),
            GetPreclaimsCommand::register(),
            TrackWorldCommand::register(),
            ClaimCommand::register(),
            StatusCommand::register(),
            StatusReportCommand::register(),
            UnclaimCommand::register(),
            MarkFreeCommand::register(),
            PublicCommand::register(),
            UnclaimedCommand::register(),
            ClaimedCommand::register(),
            FinishWorldCommand::register(),
            ReschedulePreclaimsCommand::register(),
            CancelPreclaimsCommand::register(),
            WorldsCommand::register(),
            DoneCommand::register(),
            BulkStatusCommand::register(),
            FindCommand::register(),
            RegisterCommandsCommand::register(),
            UnpreclaimCommand::register(),
            NewRealityCommand::register(),
        ],
    )
    .await
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
            BulkStatusCommand::NAME => BulkStatusCommand::execute(bot, ctx, command).await,
            FindCommand::NAME => FindCommand::execute(bot, ctx, command).await,
            RegisterCommandsCommand::NAME => RegisterCommandsCommand::execute(bot, ctx, command).await,
            UnpreclaimCommand::NAME => UnpreclaimCommand::execute(bot, ctx, command).await,
            NewRealityCommand::NAME => NewRealityCommand::execute(bot, ctx, command).await,
            _ => (),
        },
        Interaction::Component(component) => {
            if let Some((_, rest)) = component.data.custom_id.split_once("view-preclaims-") {
                ViewPreclaimsCommand::handle_interraction(bot, ctx, &component, rest).await;
            } else if let Some((_, rest)) = component.data.custom_id.split_once("unclaimed-") {
                UnclaimedCommand::handle_interraction(bot, ctx, &component, rest).await;
            } else if let Some((_, rest)) = component.data.custom_id.split_once("status-report-") {
                StatusReportCommand::handle_interraction(bot, ctx, &component, rest).await;
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
            BulkStatusCommand::NAME => BulkStatusCommand::autocomplete(bot, ctx, interaction).await,
            FindCommand::NAME => FindCommand::autocomplete(bot, ctx, interaction).await,
            NewRealityCommand::NAME => NewRealityCommand::autocomplete(bot, ctx, interaction).await,
            _ => (),
        },
        _ => (),
    }
}

pub trait Command {
    const NAME: &'static str;

    fn register() -> CreateCommand;
    async fn execute(bot: &Bot, ctx: Context, command: CommandInteraction);
    async fn autocomplete(_bot: &Bot, ctx: Context, command: CommandInteraction) {
        println!("Request for autocomplete for command {}", Self::NAME);
        command.no_autocomplete(&ctx).await;
    }
}
