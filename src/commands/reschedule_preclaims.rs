use std::time::{SystemTime, UNIX_EPOCH};

use serenity::all::{AutocompleteOption, CommandInteraction, CommandOptionType, CommandType, Context, CreateCommand, CreateCommandOption, ResolvedOption, ResolvedValue};
use sqlx::query;

use crate::{autocomplete::Autocomplete, commands::Command, util::SimpleReply, Bot};

pub struct ReschedulePreclaimsCommand {}

impl Command for ReschedulePreclaimsCommand {
    const NAME: &'static str = "reschedule-preclaims";

    fn register() -> CreateCommand {
        CreateCommand::new(Self::NAME)
            .description("Sets a new preclaim end for a world")
            .kind(CommandType::ChatInput)
            .add_option(CreateCommandOption::new(CommandOptionType::String, "world", "Name of the world").required(true).set_autocomplete(true))
            .add_option(CreateCommandOption::new(CommandOptionType::Integer, "preclaim-end", "Time preclaims close, as UNIX timestamp").required(true))
    }

    async fn execute(bot: &Bot, ctx: Context, command: CommandInteraction) {
        let user = command.user.id;

        if !bot.admins.contains(&user) {
            command.simple_reply(&ctx, "You do not have permission to use this command").await;
            return;
        }

        let mut world = "";
        let mut preclaim_end = 0;

        for ResolvedOption { name: option_name, value, .. } in command.data.options() {
            match (option_name, value) {
                ("world", ResolvedValue::String(value)) => world = value,
                ("preclaim-end", ResolvedValue::Integer(value)) => preclaim_end = value,
                _ => (),
            }
        }

        if world.is_empty() {
            command.simple_reply(&ctx, "A world is required").await;
            return;
        }

        let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();

        if current_time.as_secs() > preclaim_end as u64 {
            command.simple_reply(&ctx, "The specified preclaim end is in the past").await;
            return;
        }

        if query!("UPDATE worlds SET preclaim_end = ? WHERE name = ?", preclaim_end, world).execute(&bot.db).await.is_ok() {
            command.simple_reply(&ctx, format!("Successfully set preclaims to end at <t:{preclaim_end}:f>")).await;
        } else {
            command.simple_reply(&ctx, "Failed to set preclaim end").await;
        }

        let _ = command.defer_ephemeral(&ctx.http).await;
    }

    async fn autocomplete(bot: &Bot, ctx: Context, interaction: CommandInteraction) {
        match interaction.data.autocomplete() {
            Some(AutocompleteOption { name: "world", value, .. }) => bot.autocomplete_preclaim_worlds(ctx, &interaction, value).await,
            Some(_) | None => {
                interaction.no_autocomplete(&ctx).await;
            }
        }
    }
}
