use serenity::all::{CommandInteraction, CommandOptionType, CommandType, Context, CreateCommand, CreateCommandOption, EditInteractionResponse, ResolvedOption, ResolvedValue};
use sqlx::query;

use crate::{util::SimpleReply, Bot, Command};

pub struct DoneCommand {}

impl Command for DoneCommand {
    const NAME: &'static str = "done";

    fn register() -> CreateCommand {
        CreateCommand::new(Self::NAME)
            .description("Mark one of your slots as done to free up your claim immediately")
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
}
