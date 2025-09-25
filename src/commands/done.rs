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
        let user = i64::from(command.user.id);
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

        let Some(player) = bot.get_player(user, &command.user.name).await else {
            command.simple_reply(&ctx, "Failed to get user").await;
            return;
        };

        if let Ok(response) = query!(
            "UPDATE tracked_slots SET status = 4 WHERE name = ? AND id IN (SELECT slot FROM claims WHERE player = ?) AND world IN (SELECT id FROM tracked_worlds WHERE name = ?)",
            slot,
            player.id,
            world
        )
        .execute(&bot.db)
        .await
        {
            if response.rows_affected() == 0 {
                let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().content("Failed to mark slot as done")).await;
            } else {
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
