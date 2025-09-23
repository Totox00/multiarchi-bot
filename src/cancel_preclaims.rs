use serenity::all::{CommandInteraction, CommandOptionType, CommandType, Context, CreateCommand, CreateCommandOption, ResolvedOption, ResolvedValue};
use sqlx::query;

use crate::{util::SimpleReply, Bot, Command};

pub struct CancelPreclaimsCommand {}

impl Command for CancelPreclaimsCommand {
    const NAME: &'static str = "cancel-preclaims";

    fn register() -> CreateCommand {
        CreateCommand::new(Self::NAME)
            .description("Cancels current preclaims for a world")
            .kind(CommandType::ChatInput)
            .add_option(CreateCommandOption::new(CommandOptionType::String, "world", "Name of the world").required(true))
    }

    async fn execute(bot: &Bot, ctx: Context, command: CommandInteraction) {
        let user = command.user.id;

        if !bot.admins.contains(&user) {
            command.simple_reply(&ctx, "You do not have permission to use this command").await;
            return;
        }

        let mut world = "";

        for ResolvedOption { name: option_name, value, .. } in command.data.options() {
            if let ("world", ResolvedValue::String(value)) = (option_name, value) {
                world = value
            }
        }

        if world.is_empty() {
            command.simple_reply(&ctx, "A world is required").await;
            return;
        }

        if query!("DELETE FROM worlds WHERE name = ?", world).execute(&bot.db).await.is_ok() {
            command.simple_reply(&ctx, format!("Cancelled preclaims for {world}")).await;
        } else {
            command.simple_reply(&ctx, "Failed to cancel preclaims").await;
        }
    }
}
