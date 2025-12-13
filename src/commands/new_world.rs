use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serenity::all::{
    AutocompleteOption, CommandInteraction, CommandOptionType, CommandType, Context, CreateCommand, CreateCommandOption, CreateInteractionResponse, CreateInteractionResponseMessage, CreateMessage,
    EditInteractionResponse, ResolvedOption, ResolvedValue,
};
use sqlx::query;
use tokio::{spawn, time::sleep};

use crate::{autocomplete::Autocomplete, commands::Command, util::SimpleReply, Bot};

pub struct NewWorldCommand {}

impl Command for NewWorldCommand {
    const NAME: &'static str = "new-world";

    fn register() -> CreateCommand {
        CreateCommand::new(Self::NAME)
            .description("Creates a new world")
            .kind(CommandType::ChatInput)
            .add_option(CreateCommandOption::new(CommandOptionType::String, "name", "Name of the new world").required(true))
            .add_option(CreateCommandOption::new(CommandOptionType::Integer, "preclaim-end", "Time preclaims close, as UNIX timestamp").required(true))
            .add_option(CreateCommandOption::new(CommandOptionType::Attachment, "slot-file", "Output file from clean_yamls").required(true))
            .add_option(
                CreateCommandOption::new(CommandOptionType::String, "reality", "Name of the reality")
                    .required(false)
                    .set_autocomplete(true),
            )
            .add_option(CreateCommandOption::new(CommandOptionType::String, "message", "Additional message to display").required(false))
            .add_option(CreateCommandOption::new(CommandOptionType::Boolean, "ping", "If @preclaims should be pinged. Defaults to true").required(false))
    }

    async fn execute(bot: &Bot, ctx: Context, command: CommandInteraction) {
        let user = command.user.id;

        if !bot.admins.contains(&user) {
            command.simple_reply(&ctx, "You do not have permission to use this command").await;
            return;
        }

        let mut name = "";
        let mut reality_name = None;
        let mut preclaim_end = 0;
        let mut slot_file = None;
        let mut message = None;
        let mut ping = true;

        for ResolvedOption { name: option_name, value, .. } in command.data.options() {
            match (option_name, value) {
                ("name", ResolvedValue::String(value)) => name = value,
                ("reality", ResolvedValue::String(value)) => reality_name = Some(value),
                ("preclaim-end", ResolvedValue::Integer(value)) => preclaim_end = value,
                ("slot-file", ResolvedValue::Attachment(value)) => slot_file = Some(value),
                ("message", ResolvedValue::String(value)) => message = Some(value),
                ("ping", ResolvedValue::Boolean(value)) => ping = value,
                _ => (),
            }
        }

        if name.is_empty() {
            command.simple_reply(&ctx, "A world name is required").await;
            return;
        }

        let reality = if let Some(reality_name) = reality_name {
            if let Ok(response) = query!("SELECT id FROM realities WHERE name = ? LIMIT 1", reality_name).fetch_one(&bot.db).await {
                Some(response.id)
            } else {
                command.simple_reply(&ctx, "Failed to get reality").await;
                return;
            }
        } else {
            None
        };

        let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
        let current_secs = current_time.as_secs();

        if current_secs > preclaim_end as u64 {
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

        if let Ok(response) = query!("SELECT name FROM tracked_worlds WHERE id IN (SELECT world FROM tracked_slots WHERE status < 2)")
            .fetch_all(&bot.db)
            .await
        {
            for record in response {
                bot.update_scrape(&record.name).await;
            }
        }

        bot.push_needed().await;
        bot.update_unspent_points().await;

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

        if let Ok(response) = query!("INSERT INTO worlds (name, preclaim_end, reality) VALUES (?, ?, ?) RETURNING id", name, preclaim_end, reality)
            .fetch_one(&bot.db)
            .await
        {
            let slot_len = slots.len();
            for (name, games, notes, points) in slots {
                if query!("INSERT INTO slots (world, name, games, notes, points) VALUES (?, ?, ?, ?, ?)", response.id, name, games, notes, points)
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

            if let Some(preclaims_channel) = Bot::preclaims_channel(&ctx).await {
                let _ = preclaims_channel
                    .send_message(
                        &ctx,
                        CreateMessage::new().content(format!(
                            "{}{slot_len} slots available for preclaim in {name}{} until <t:{preclaim_end}:f>. Use `/view-preclaims` to view them and make preclaims.{}",
                            if ping { "[<@&1342190668231213176>] " } else { "" },
                            if let Some(reality_name) = reality_name { format!(" in {reality_name}") } else { String::new() },
                            if let Some(message) = message { format!(" {message}") } else { String::new() }
                        )),
                    )
                    .await;
            }

            let owned_name = name.to_owned();
            spawn(async move {
                sleep(Duration::from_secs(preclaim_end as u64 - current_secs)).await;

                if let Some(system_channel) = Bot::system_channel(&ctx).await {
                    let _ = system_channel
                        .send_message(&ctx, CreateMessage::new().content(format!("[<@&1399971928076455946>] Preclaims are closed for {owned_name}.")))
                        .await;
                }
            });
        } else {
            let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().content("Failed to create new world")).await;
        }
    }

    async fn autocomplete(bot: &Bot, ctx: Context, interaction: CommandInteraction) {
        match interaction.data.autocomplete() {
            Some(AutocompleteOption { name: "reality", value, .. }) => bot.autocomplete_realities(ctx, &interaction, value).await,
            Some(_) | None => {
                interaction.no_autocomplete(&ctx).await;
            }
        }
    }
}

async fn fetch_slot_file(url: &str) -> Result<String, reqwest::Error> {
    reqwest::get(url).await?.text().await
}

fn parse_slot_file(content: &str) -> Option<Vec<(String, String, String, String)>> {
    let mut out = vec![];

    for [name, games, notes, points] in content.lines().array_chunks() {
        out.push((name.to_owned(), games.to_owned(), notes.to_owned(), points.to_owned()));
    }

    Some(out)
}
