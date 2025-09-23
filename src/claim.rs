use serenity::all::{CommandInteraction, CommandOptionType, CommandType, Context, CreateCommand, CreateCommandOption, EditInteractionResponse, ResolvedOption, ResolvedValue};
use sqlx::query;

use crate::{util::SimpleReply, Bot, Command};

pub struct ClaimCommand {}

impl Command for ClaimCommand {
    const NAME: &'static str = "claim";

    fn register() -> CreateCommand {
        CreateCommand::new(Self::NAME)
            .description("Claim a slot in a world")
            .kind(CommandType::ChatInput)
            .add_option(CreateCommandOption::new(CommandOptionType::String, "world", "Name of the world").required(true))
            .add_option(CreateCommandOption::new(CommandOptionType::String, "slot", "Name of the slot").required(true))
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

        let player = if let Some(player) = bot.get_player(user).await {
            player
        } else {
            command.simple_reply(&ctx, "Failed to get user").await;
            return;
        };

        let mut must_be_free = false;

        if let Ok(response) = query!("SELECT claims FROM current_claims WHERE player = ? LIMIT 1", player.id).fetch_optional(&bot.db).await {
            if response.is_some_and(|record| record.claims >= player.claims) {
                must_be_free = true;
            }
        } else {
            command.simple_reply(&ctx, "Failed to get current claims").await;
            return;
        }

        let (slot_id, free) = if let Ok(response) = query!(
            "SELECT id, free FROM tracked_slots WHERE name = ? AND world in (SELECT id FROM tracked_worlds WHERE name = ?) LIMIT 1",
            slot,
            world
        )
        .fetch_one(&bot.db)
        .await
        {
            (response.id, response.free > 0)
        } else {
            command.simple_reply(&ctx, "Failed to get slot").await;
            return;
        };

        if must_be_free && !free {
            command.simple_reply(&ctx, "You are already at claim limit").await;
            return;
        }

        if let Ok(response) = query!("SELECT id FROM claims WHERE slot = ? LIMIT 1", slot_id).fetch_optional(&bot.db).await {
            if response.is_some() {
                command.simple_reply(&ctx, "Slot is already claimed").await;
                return;
            }
        } else {
            command.simple_reply(&ctx, "Failed to get claim status for slot").await;
            return;
        }

        let _ = command.defer_ephemeral(&ctx.http).await;

        if query!("INSERT INTO claims (slot, player) VALUES (?, ?)", slot_id, player.id).execute(&bot.db).await.is_ok() {
            if query!("DELETE FROM preclaims WHERE player = ? AND status = 0", player.id).execute(&bot.db).await.is_err() {
                println!("Failed to remove old preclaims from {}", player.id)
            }

            let _ = command
                .edit_response(&ctx.http, EditInteractionResponse::new().content(format!("Successfully claimed {slot} in {world}")))
                .await;
        } else {
            let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().content("Failed to claim slot")).await;
        }
    }
}
