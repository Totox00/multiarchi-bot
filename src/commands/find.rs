use serenity::all::{AutocompleteOption, CommandInteraction, CommandOptionType, CommandType, Context, CreateCommand, CreateCommandOption, ResolvedOption, ResolvedValue};
use sqlx::query;

use crate::{autocomplete::Autocomplete, commands::Command, util::SimpleReply, Bot};

pub struct FindCommand {}

impl Command for FindCommand {
    const NAME: &'static str = "find";

    fn register() -> CreateCommand {
        CreateCommand::new(Self::NAME)
            .description("Find the owner of a slot")
            .kind(CommandType::ChatInput)
            .add_option(CreateCommandOption::new(CommandOptionType::String, "world", "Name of the world").required(true).set_autocomplete(true))
            .add_option(CreateCommandOption::new(CommandOptionType::String, "slot", "Name of the slot").required(true).set_autocomplete(true))
    }

    async fn execute(bot: &Bot, ctx: Context, command: CommandInteraction) {
        let mut world = "";
        let mut slot = "";

        for ResolvedOption { name: option_name, value, .. } in command.data.options() {
            match (option_name, value) {
                ("world", ResolvedValue::String(value)) => world = value,
                ("slot", ResolvedValue::String(value)) => slot = value,
                _ => (),
            }
        }

        if world.is_empty() {
            command.simple_reply(&ctx, "A world is required").await;
            return;
        }

        if slot.is_empty() {
            command.simple_reply(&ctx, "A slot is required").await;
            return;
        }

        let slot_id = if let Ok(response) = query!(
            "SELECT id FROM tracked_slots WHERE name = ? AND world IN (SELECT id FROM tracked_worlds WHERE name = ?) LIMIT 1",
            slot,
            world
        )
        .fetch_one(&bot.db)
        .await
        {
            response.id
        } else {
            command.simple_reply(&ctx, "Failed to find slot").await;
            return;
        };

        if let Ok(response) = query!("SELECT snowflake FROM players WHERE id IN (SELECT player FROM claims WHERE slot = ?) LIMIT 1", slot_id)
            .fetch_one(&bot.db)
            .await
        {
            command.simple_reply(&ctx, format!("{slot} in {world} is claimed by <@{}>", response.snowflake)).await;
        } else {
            command.simple_reply(&ctx, "Failed to find player").await;
        }
    }

    async fn autocomplete(bot: &Bot, ctx: Context, interaction: CommandInteraction) {
        match interaction.data.autocomplete() {
            Some(AutocompleteOption { name: "world", value, .. }) => bot.autocomplete_worlds(ctx, &interaction, value).await,
            Some(AutocompleteOption { name: "slot", value, .. }) => {
                let mut world = None;
                for ResolvedOption { name: option_name, value, .. } in interaction.data.options() {
                    if let ("world", ResolvedValue::String(value)) = (option_name, value) {
                        world = Some(value)
                    }
                }

                bot.autocomplete_slots(ctx, &interaction, value, world).await;
            }
            Some(_) | None => {
                interaction.no_autocomplete(&ctx).await;
            }
        }
    }
}
