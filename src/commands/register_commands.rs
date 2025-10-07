use serenity::all::{CommandInteraction, CommandType, Context, CreateCommand};

use crate::{
    commands::{register_all, Command},
    util::SimpleReply,
    Bot,
};

pub struct RegisterCommandsCommand {}

impl Command for RegisterCommandsCommand {
    const NAME: &'static str = "register-commands";

    fn register() -> CreateCommand {
        CreateCommand::new(Self::NAME).description("Registers all commands").kind(CommandType::ChatInput)
    }

    async fn execute(bot: &Bot, ctx: Context, command: CommandInteraction) {
        let user = command.user.id;

        if !bot.admins.contains(&user) {
            command.simple_reply(&ctx, "You do not have permission to use this command").await;
            return;
        }

        if let Err(err) = register_all(&ctx).await {
            command.simple_reply(&ctx, format!("Failed to register commands with error {err:?}")).await;
        } else {
            command.simple_reply(&ctx, "Successfully registered commands").await;
        }
    }
}
