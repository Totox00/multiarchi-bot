use serenity::all::{AutocompleteOption, CommandInteraction, CommandOptionType, CommandType, Context, CreateCommand, CreateCommandOption, EditInteractionResponse, ResolvedOption, ResolvedValue};
use sqlx::query;

use crate::{autocomplete::Autocomplete, commands::Command, util::SimpleReply, Bot};

pub struct DoneCommand {}

impl Command for DoneCommand {
    const NAME: &'static str = "done";

    fn register() -> CreateCommand {
        CreateCommand::new(Self::NAME)
            .description("Mark one of your slots as done to free up your claim immediately")
            .kind(CommandType::ChatInput)
            .add_option(CreateCommandOption::new(CommandOptionType::String, "world", "Name of the world").required(true).set_autocomplete(true))
            .add_option(CreateCommandOption::new(CommandOptionType::String, "slot", "Name of the slot").required(true).set_autocomplete(true))
    }

    async fn execute(bot: &Bot, ctx: Context, command: CommandInteraction) {
        if !bot.privileged.contains(&command.user.id) {
            command.simple_reply(&ctx, "You do not have permission to use this command").await;
            return;
        }

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

        let _ = command.defer_ephemeral(&ctx.http).await;

        if let Ok(response) = query!(
            "UPDATE tracked_slots SET status = 4 WHERE name = ? AND world IN (SELECT id FROM tracked_worlds WHERE name = ?)",
            slot,
            world
        )
        .execute(&bot.db)
        .await
        {
            if response.rows_affected() == 0 {
                let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().content("Failed to mark slot as done")).await;
            } else {
                bot.log(&format!("Slot {slot} in {world} was marked done by {}", command.user.name));
                let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().content("Successfully marked slot as done")).await;
            }
        } else {
            let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().content("Failed to mark slot as done")).await;
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
