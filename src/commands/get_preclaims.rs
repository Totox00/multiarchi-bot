use std::{
    collections::HashMap,
    time::{SystemTime, UNIX_EPOCH},
};

use rand::{seq::SliceRandom, thread_rng};
use serenity::{
    all::{AutocompleteOption, CommandInteraction, CommandOptionType, CommandType, Context, CreateCommand, CreateCommandOption, ResolvedOption, ResolvedValue},
    futures::future::join_all,
};
use sqlx::query;

use crate::{autocomplete::Autocomplete, commands::Command, util::SimpleReply, Bot};

pub struct GetPreclaimsCommand {}

impl Command for GetPreclaimsCommand {
    const NAME: &'static str = "get-preclaims";

    fn register() -> CreateCommand {
        CreateCommand::new(Self::NAME)
            .description("Gets preclaims for a world")
            .kind(CommandType::ChatInput)
            .add_option(CreateCommandOption::new(CommandOptionType::String, "world", "Name of the world").required(true).set_autocomplete(true))
    }

    async fn execute(bot: &Bot, ctx: Context, command: CommandInteraction) {
        let user = command.user.id;

        if !bot.admins.contains(&user) {
            command.simple_reply(&ctx, "You do not have permission to use this command").await;
            return;
        }

        let mut name = "";

        for ResolvedOption { name: option_name, value, .. } in command.data.options() {
            if let ("world", ResolvedValue::String(value)) = (option_name, value) {
                name = value
            }
        }

        if name.is_empty() {
            command.simple_reply(&ctx, "A world name is required").await;
            return;
        }

        if let Ok(response) = query!("SELECT preclaim_end FROM worlds WHERE name = ? LIMIT 1", name).fetch_one(&bot.db).await {
            let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();

            if current_time.as_secs() < response.preclaim_end as u64 {
                command.simple_reply(&ctx, format!("Preclaims end at <t:{}:f>", response.preclaim_end)).await;
                return;
            }
        } else {
            command.simple_reply(&ctx, "Failed to get world").await;
            return;
        }

        let selected_preclaims = if let Some(selected_preclaims) = resolve_preclaims(bot, name).await {
            selected_preclaims
        } else {
            command.simple_reply(&ctx, "Failed to resolve preclaims").await;
            return;
        };

        if selected_preclaims.is_empty() {
            command.simple_reply(&ctx, "World had no preclaims").await;
        } else {
            command
                .simple_reply(
                    &ctx,
                    format!(
                        "```\n{}\n```",
                        join_all(selected_preclaims.into_iter().map(async |(slot, player)| (
                            query!("SELECT name FROM slots WHERE id = ? LIMIT 1", slot).fetch_one(&bot.db).await,
                            query!("SELECT snowflake FROM players WHERE id = ? LIMIT 1", player).fetch_one(&bot.db).await
                        )),)
                        .await
                        .into_iter()
                        .filter_map(|(slot_response, player_response)| if let (Ok(slot_record), Ok(player_record)) = (slot_response, player_response) {
                            Some(format!("<@{}> {} is yours", player_record.snowflake, slot_record.name))
                        } else {
                            None
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                    ),
                )
                .await;
        }
    }

    async fn autocomplete(bot: &Bot, ctx: Context, interaction: CommandInteraction) {
        match interaction.data.autocomplete() {
            Some(AutocompleteOption { name: "world", value, .. }) => bot.autocomplete_preclaim_worlds(ctx, &interaction, value).await,
            Some(_) | None => {
                interaction.no_autocomplete(&ctx).await;
            }
        }
    }
}

pub async fn resolve_preclaims(bot: &Bot, name: &str) -> Option<Vec<(i64, i64)>> {
    if query!("SELECT resolved_preclaims FROM worlds WHERE name = ? LIMIT 1", name)
        .fetch_one(&bot.db)
        .await
        .ok()?
        .resolved_preclaims
        > 0
    {
        Some(
            query!(
                "SELECT slot, player FROM preclaims WHERE status = 2 AND slot IN (SELECT id FROM slots WHERE world IN (SELECT id FROM worlds WHERE name = ?))",
                name
            )
            .fetch_all(&bot.db)
            .await
            .ok()?
            .into_iter()
            .map(|record| (record.slot, record.player))
            .collect(),
        )
    } else {
        let mut preclaims: HashMap<i64, Vec<i64>> = HashMap::new();

        for record in query!(
            "UPDATE preclaims SET status = 1 WHERE slot IN (SELECT id FROM slots WHERE world IN (SELECT id FROM worlds WHERE name = ?)) RETURNING slot, player",
            name
        )
        .fetch_all(&bot.db)
        .await
        .ok()?
        {
            if bot.can_preclaim_slot(record.player, record.slot).await.is_ok() {
                preclaims.entry(record.slot).and_modify(|vec| vec.push(record.player)).or_insert(vec![record.player]);
            }
        }

        let selected_preclaims: Vec<_> = {
            let mut rng = thread_rng();
            preclaims
                .iter()
                .map(|(slot, players)| (slot, players.choose(&mut rng)))
                .filter_map(|(slot, player)| player.map(|player| (*slot, *player)))
                .collect()
        };

        for (slot, player) in &selected_preclaims {
            if query!("UPDATE preclaims SET status = 2 WHERE slot = ? AND player = ?", slot, player).execute(&bot.db).await.is_err() {
                println!("Failed to set preclaim status for player {player} for slot {slot}");
            }
        }

        if query!("UPDATE worlds SET resolved_preclaims = 1 WHERE name = ?", name).execute(&bot.db).await.is_err() {
            println!("Failed to mark preclaims as resolved for world {name}");
        }

        Some(selected_preclaims)
    }
}
