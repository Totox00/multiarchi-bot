use serenity::all::{CommandInteraction, CommandOptionType, CommandType, Context, CreateCommand, CreateCommandOption, ResolvedOption, ResolvedValue};
use sqlx::query;

use crate::{commands::Command, util::SimpleReply, Bot};

pub struct TransferPointsCommand {}

impl Command for TransferPointsCommand {
    const NAME: &'static str = "transfer-points";

    fn register() -> CreateCommand {
        CreateCommand::new(Self::NAME)
            .description("Transfer points to someone else")
            .kind(CommandType::ChatInput)
            .add_option(CreateCommandOption::new(CommandOptionType::User, "player", "Player to transfer points to").required(false))
    }

    async fn execute(bot: &Bot, ctx: Context, command: CommandInteraction) {
        let user = i64::from(command.user.id);

        let mut target = None;

        for ResolvedOption { name: option_name, value, .. } in command.data.options() {
            if let ("player", ResolvedValue::User(value, _)) = (option_name, value) {
                target = Some(value)
            }
        }

        if let Some(target) = target {
            let target_snowflake = i64::from(target.id);
            let Some(target_id) = query!("SELECT id FROM players WHERE snowflake = ? LIMIT 1", target_snowflake)
                .fetch_one(&bot.db)
                .await
                .ok()
                .map(|record| record.id)
            else {
                command.simple_reply(&ctx, "Failed to get recipient").await;
                return;
            };
            if query!("UPDATE players SET transfer_to = ? WHERE snowflake = ?", target_id, user).execute(&bot.db).await.is_ok() {
                command.simple_reply(&ctx, format!("Began transferring points to <@{}>", target_snowflake)).await;
            } else {
                command.simple_reply(&ctx, "Failed to transfer points").await;
            }
        } else if query!("UPDATE players SET transfer_to = NULL WHERE snowflake = ?", user).execute(&bot.db).await.is_ok() {
            command.simple_reply(&ctx, "You are no longer transferring your earned points").await;
        } else {
            command.simple_reply(&ctx, "Failed to cancel transferring of points").await;
        }
    }
}
