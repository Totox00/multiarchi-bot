use std::collections::HashMap;

use rand::{seq::SliceRandom, thread_rng};
use serenity::{
    all::{CommandInteraction, CommandOptionType, CommandType, Context, CreateCommand, CreateCommandOption, ResolvedOption, ResolvedValue},
    futures::future::join_all,
};
use sqlx::query;

use crate::{util::SimpleReply, Bot};

pub struct GetPreclaimsCommand {}

impl GetPreclaimsCommand {
    pub fn register() -> CreateCommand {
        CreateCommand::new("get-preclaims")
            .description("Gets preclaims for a world")
            .kind(CommandType::ChatInput)
            .add_option(CreateCommandOption::new(CommandOptionType::String, "world-name", "Name of the world"))
    }

    pub async fn execute(bot: &Bot, ctx: Context, command: CommandInteraction) {
        let user = command.user.id;

        if !bot.admins.contains(&user) {
            command.simple_reply(&ctx, "You do not have permission to use this command").await;
            return;
        }

        let mut name = "";

        for ResolvedOption { name: option_name, value, .. } in command.data.options() {
            if let ("world-name", ResolvedValue::String(value)) = (option_name, value) {
                name = value
            }
        }

        if name.is_empty() {
            command.simple_reply(&ctx, "A world name is required").await;
            return;
        }

        let preclaims = if let Ok(response) = query!(
            "UPDATE preclaims SET old = 1 WHERE slot IN (SELECT id FROM slots WHERE world IN (SELECT id FROM worlds WHERE name = ?)) RETURNING slot, user_snowflake",
            name
        )
        .fetch_all(&bot.db)
        .await
        {
            let mut preclaims: HashMap<i64, Vec<i64>> = HashMap::new();

            for record in response {
                if let Some(slot) = record.slot {
                    preclaims.entry(slot).and_modify(|vec| vec.push(record.user_snowflake)).or_insert(vec![record.user_snowflake]);
                }
            }
            preclaims
        } else {
            command.simple_reply(&ctx, "Failed to get preclaims").await;
            return;
        };

        let selected_preclaims: Vec<_> = {
            let mut rng = thread_rng();
            preclaims.into_iter().filter_map(|(slot, users)| users.choose(&mut rng).copied().map(|user| (slot, user))).collect()
        };

        if selected_preclaims.is_empty() {
            command.simple_reply(&ctx, "World had no preclaims").await;
        } else {
            command
                .simple_reply(
                    &ctx,
                    format!(
                        "```\n{}\n```",
                        join_all(
                            selected_preclaims
                                .into_iter()
                                .map(async |(slot, user)| (user, query!("SELECT name FROM slots WHERE id = ? LIMIT 1", slot).fetch_one(&bot.db).await)),
                        )
                        .await
                        .into_iter()
                        .filter_map(|(user, response)| if let Ok(record) = response { Some(format!("<@{user}> {} is yours", record.name)) } else { None })
                        .collect::<Vec<_>>()
                        .join("\n")
                    ),
                )
                .await;
        }
    }
}
