use serenity::all::{CommandInteraction, CommandOptionType, CommandType, Context, CreateCommand, CreateCommandOption, EditInteractionResponse, ResolvedOption, ResolvedValue};
use sqlx::query;

use crate::{commands::Command, util::SimpleReply, Bot};

pub struct NewRealityCommand {}

impl Command for NewRealityCommand {
    const NAME: &'static str = "new-reality";

    fn register() -> CreateCommand {
        CreateCommand::new(Self::NAME)
            .description("Creates a new reality")
            .kind(CommandType::ChatInput)
            .add_option(CreateCommandOption::new(CommandOptionType::String, "name", "Name of the new world").required(true))
            .add_option(CreateCommandOption::new(CommandOptionType::Integer, "max-claims", "Maximum claims within reality").required(true))
    }

    async fn execute(bot: &Bot, ctx: Context, command: CommandInteraction) {
        let user = command.user.id;

        if !bot.admins.contains(&user) {
            command.simple_reply(&ctx, "You do not have permission to use this command").await;
            return;
        }

        let mut name = "";
        let mut max_claims = 0;

        for ResolvedOption { name: option_name, value, .. } in command.data.options() {
            match (option_name, value) {
                ("name", ResolvedValue::String(value)) => name = value,
                ("max-claims", ResolvedValue::Integer(value)) => max_claims = value,
                _ => (),
            }
        }

        if name.is_empty() {
            command.simple_reply(&ctx, "A world name is required").await;
            return;
        }

        if max_claims < 1 {
            command.simple_reply(&ctx, "Max claims must be a positive integer").await;
            return;
        }

        let _ = command.defer_ephemeral(&ctx.http).await;

        if query!("INSERT INTO realities (name, max_claims) VALUES (?, ?)", name, max_claims).execute(&bot.db).await.is_ok() {
            let _ = command
                .edit_response(
                    &ctx.http,
                    EditInteractionResponse::new().content(format!("Created new reality {name} with a claim limit of {max_claims}")),
                )
                .await;
        } else {
            let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().content("Failed to create new world")).await;
        }
    }
}
