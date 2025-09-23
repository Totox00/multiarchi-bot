use serenity::all::{CommandInteraction, CommandOptionType, CommandType, Context, CreateCommand, CreateCommandOption, EditInteractionResponse, ResolvedOption, ResolvedValue};
use sqlx::query;

use crate::{util::SimpleReply, Bot, Command};

pub struct MarkFreeCommand {}

impl Command for MarkFreeCommand {
    const NAME: &'static str = "mark-free";

    fn register() -> CreateCommand {
        CreateCommand::new(Self::NAME)
            .description("Marks a slot in a world as a free claim")
            .kind(CommandType::ChatInput)
            .add_option(CreateCommandOption::new(CommandOptionType::String, "world", "Name of the world").required(true))
            .add_option(CreateCommandOption::new(CommandOptionType::String, "slot", "Name of the slot").required(true))
    }

    async fn execute(bot: &Bot, ctx: Context, command: CommandInteraction) {
        let user = command.user.id;

        if !bot.admins.contains(&user) {
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

        if query!(
            "UPDATE tracked_slots SET free = 1 WHERE name = ? AND world IN (SELECT id FROM tracked_worlds WHERE name = ?)",
            slot,
            world
        )
        .execute(&bot.db)
        .await
        .is_ok()
        {
            let _ = command
                .edit_response(&ctx.http, EditInteractionResponse::new().content(format!("Successfully marked {slot} in {world} as a free claim")))
                .await;
        } else {
            let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().content("Failed to mark slot as a free claim")).await;
        }
    }
}
