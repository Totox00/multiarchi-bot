use std::iter::once;

use serenity::all::{
    AutocompleteOption, CommandInteraction, CommandOptionType, CommandType, Context, CreateCommand, CreateCommandOption, CreateMessage, EditInteractionResponse, ResolvedOption, ResolvedValue,
};
use sqlx::query;

use crate::{autocomplete::Autocomplete, commands::Command, util::SimpleReply, Bot};

pub struct StatusCommand {}

impl Command for StatusCommand {
    const NAME: &'static str = "status";

    fn register() -> CreateCommand {
        CreateCommand::new(Self::NAME)
            .description("Report the status of a slot")
            .kind(CommandType::ChatInput)
            .add_option(CreateCommandOption::new(CommandOptionType::String, "world", "Name of the world").required(true).set_autocomplete(true))
            .add_option(CreateCommandOption::new(CommandOptionType::String, "slot", "Name of the slot").required(true).set_autocomplete(true))
            .add_option(
                CreateCommandOption::new(CommandOptionType::String, "description", "Status description")
                    .required(true)
                    .set_autocomplete(true),
            )
    }

    async fn execute(bot: &Bot, ctx: Context, command: CommandInteraction) {
        let mut world = "";
        let mut slot = "";
        let mut description = "";

        for ResolvedOption { name: option_name, value, .. } in command.data.options() {
            match (option_name, value) {
                ("world", ResolvedValue::String(value)) => world = value,
                ("slot", ResolvedValue::String(value)) => slot = value,
                ("description", ResolvedValue::String(value)) => description = value,
                _ => (),
            }
        }

        let slot_id = if let Ok(response) = query!("SELECT id FROM tracked_slots WHERE name = ? AND world in (SELECT id FROM tracked_worlds WHERE name = ?)", slot, world)
            .fetch_one(&bot.db)
            .await
        {
            response.id
        } else {
            command.simple_reply(&ctx, "Failed to get slot").await;
            return;
        };

        let Some(player) = bot.get_player(i64::from(command.user.id), &command.user.name).await else {
            command.simple_reply(&ctx, "Failed to get user").await;
            return;
        };

        let _ = command.defer_ephemeral(&ctx.http).await;

        if query!("INSERT INTO updates (slot, player, description) VALUES (?, ?, ?)", slot_id, player.id, description)
            .execute(&bot.db)
            .await
            .is_ok()
        {
            if let Some(status_channel) = Bot::status_channel(&ctx).await {
                let _ = status_channel
                    .send_message(&ctx, CreateMessage::new().content(format!("[{}] [{world}] [{slot}] {description}", command.user.display_name())))
                    .await;
            }

            let _ = command
                .edit_response(&ctx.http, EditInteractionResponse::new().content(format!("Successfully updated status of {slot} in {world}")))
                .await;
        } else {
            let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().content("Failed to update status")).await;
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

                if let Some(player) = bot.get_player(i64::from(interaction.user.id), &interaction.user.name).await {
                    bot.autocomplete_slots_claimed(ctx, &interaction, value, world, &player).await;
                } else {
                    bot.autocomplete_slots(ctx, &interaction, value, world).await;
                }
            }
            Some(AutocompleteOption { name: "description", .. }) => {
                let mut world = None;
                let mut slot = None;

                for ResolvedOption { name: option_name, value, .. } in interaction.data.options() {
                    match (option_name, value) {
                        ("world", ResolvedValue::String(value)) => world = Some(value),
                        ("slot", ResolvedValue::String(value)) => slot = Some(value),
                        _ => (),
                    }
                }

                if let (Some(world), Some(slot), Some(player)) = (world, slot, bot.get_player(i64::from(interaction.user.id), &interaction.user.name).await) {
                    if let Ok(response) = query!("SELECT description FROM updates WHERE player = ? AND slot IN (SELECT id FROM tracked_slots WHERE name = ? AND world IN (SELECT id FROM tracked_worlds WHERE name = ?)) ORDER BY timestamp DESC LIMIT 1", player.id, slot, world)
                        .fetch_one(&bot.db)
                        .await
                    {
                        let _ = interaction.autocomplete(&ctx, once(response.description)).await;
                        return;
                    }
                }
                interaction.no_autocomplete(&ctx).await;
            }
            Some(_) | None => {
                interaction.no_autocomplete(&ctx).await;
            }
        }
    }
}
