use serenity::all::{CommandInteraction, CommandType, Context, CreateCommand};
use sqlx::query;

use crate::{commands::Command, util::SimpleReply, Bot};

pub struct UnpreclaimCommand {}

impl Command for UnpreclaimCommand {
    const NAME: &'static str = "unpreclaim";

    fn register() -> CreateCommand {
        CreateCommand::new(Self::NAME).description("Removes your current preclaim").kind(CommandType::ChatInput)
    }

    async fn execute(bot: &Bot, ctx: Context, command: CommandInteraction) {
        let user = i64::from(command.user.id);

        if let Ok(response) = query!("DELETE FROM preclaims WHERE player IN (SELECT id FROM players WHERE snowflake = ?) AND slot IN (SELECT id FROM slots WHERE world IN (SELECT id FROM worlds WHERE preclaim_end > strftime('%s', 'now')))", user).execute(&bot.db).await {
            if response.rows_affected() > 0 {
                command.simple_reply(&ctx, "Successfully removed preclaim").await;
            } else {
                command.simple_reply(&ctx, "Failed to remove preclaim").await;
            }
        } else {
            command.simple_reply(&ctx, "Failed to remove preclaim").await;
        }
    }
}
