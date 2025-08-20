use std::time::{SystemTime, UNIX_EPOCH};

use serenity::all::{
    CommandInteraction, CommandOptionType, CommandType, Context, CreateCommand, CreateCommandOption, CreateInteractionResponse, CreateInteractionResponseMessage, EditInteractionResponse,
    ResolvedOption, ResolvedValue,
};
use sqlx::query;

use crate::{util::SimpleReply, Bot};

pub struct NewWorldCommand {}

impl NewWorldCommand {
    pub fn register() -> CreateCommand {
        CreateCommand::new("new-world")
            .description("Creates a new world")
            .kind(CommandType::ChatInput)
            .add_option(CreateCommandOption::new(CommandOptionType::String, "world-name", "Name of the new world"))
            .add_option(CreateCommandOption::new(CommandOptionType::Number, "preclaim-end", "Time preclaims close, as UNIX timestamp"))
            .add_option(CreateCommandOption::new(CommandOptionType::Attachment, "slot-file", "Output file from clean_yamls"))
    }

    pub async fn execute(bot: &Bot, ctx: Context, command: CommandInteraction) {
        let user = command.user.id;

        if !bot.admins.contains(&user) {
            command.simple_reply(&ctx, "You do not have permission to use this command").await;
            return;
        }

        let mut name = "";
        let mut preclaim_end = 0;
        let mut slot_file = None;

        for ResolvedOption { name: option_name, value, .. } in command.data.options() {
            match (option_name, value) {
                ("world-name", ResolvedValue::String(value)) => name = value,
                ("preclaim-end", ResolvedValue::Number(value)) => preclaim_end = value as u64,
                ("slot-file", ResolvedValue::Attachment(value)) => slot_file = Some(value),
                _ => (),
            }
        }

        if name.is_empty() {
            command.simple_reply(&ctx, "A world name is required").await;
            return;
        }

        let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();

        if current_time.as_secs() > preclaim_end {
            command.simple_reply(&ctx, "The specified preclaim end is in the past").await;
            return;
        }

        let slot_file = if let Some(slot_file) = slot_file {
            if slot_file.size > 1_000_000 {
                let _ = command
                    .create_response(
                        &ctx.http,
                        CreateInteractionResponse::Message(CreateInteractionResponseMessage::new().ephemeral(true).content("Slot file is too large")),
                    )
                    .await;
                return;
            } else {
                slot_file
            }
        } else {
            command.simple_reply(&ctx, "Slot file does not exist").await;
            return;
        };

        let _ = command.defer_ephemeral(&ctx.http).await;

        let slot_file_content = if let Ok(content) = fetch_slot_file(&slot_file.url).await {
            content
        } else {
            let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().content("Failed to fetch slot file")).await;
            return;
        };
        let slots = if let Some(slots) = parse_slot_file(&slot_file_content) {
            slots
        } else {
            let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().content("Failed to parse slot file")).await;
            return;
        };

        let preclaim_end_i64 = preclaim_end as i64;
        let user_snowflake = i64::from(user);

        if let Ok(response) = query!("INSERT INTO worlds (name, preclaim_end, creator) VALUES (?, ?, ?) RETURNING id", name, preclaim_end_i64, user_snowflake)
            .fetch_one(&bot.db)
            .await
        {
            let slot_len = slots.len();
            for (name, games, notes) in slots {
                if query!("INSERT INTO slots (world, name, games, notes) VALUES (?, ?, ?, ?)", response.id, name, games, notes)
                    .execute(&bot.db)
                    .await
                    .is_err()
                {
                    let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().content("Failed to create slots for new world")).await;
                    return;
                }
            }

            let _ = command
                .edit_response(&ctx.http, EditInteractionResponse::new().content(format!("Successfully created world {name} with {slot_len} yamls")))
                .await;
        } else {
            let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().content("Failed to create new world")).await;
        }
    }
}

async fn fetch_slot_file(url: &str) -> Result<String, reqwest::Error> {
    reqwest::get(url).await?.text().await
}

fn parse_slot_file(content: &str) -> Option<Vec<(String, String, String)>> {
    let mut out = vec![];

    for [name, games, notes] in content.lines().array_chunks() {
        out.push((name.to_owned(), games.to_owned(), notes.to_owned()));
    }

    Some(out)
}
