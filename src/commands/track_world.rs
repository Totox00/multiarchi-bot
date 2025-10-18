use std::collections::HashMap;

use phf::phf_map;
use serenity::all::{
    AutocompleteOption, CommandInteraction, CommandOptionType, CommandType, Context, CreateCommand, CreateCommandOption, CreateMessage, EditInteractionResponse, ResolvedOption, ResolvedValue,
};
use sqlx::query;

use crate::{
    autocomplete::Autocomplete,
    commands::{get_preclaims::resolve_preclaims, Command},
    scrape::{fetch_tracker, scrape},
    util::SimpleReply,
    Bot,
};

pub struct TrackWorldCommand {}

impl Command for TrackWorldCommand {
    const NAME: &'static str = "track-world";

    fn register() -> CreateCommand {
        CreateCommand::new(Self::NAME)
            .description("Tracks a new world")
            .kind(CommandType::ChatInput)
            .add_option(CreateCommandOption::new(CommandOptionType::String, "tracker", "Link to or id of the tracker for this world").required(true))
            .add_option(CreateCommandOption::new(CommandOptionType::String, "name", "Name of the world").required(true).set_autocomplete(true))
            .add_option(
                CreateCommandOption::new(CommandOptionType::String, "reality", "Name of the reality")
                    .required(false)
                    .set_autocomplete(true),
            )
            .add_option(CreateCommandOption::new(CommandOptionType::String, "import-claims", "If claims should be imported from a prior world").required(false))
            .add_option(CreateCommandOption::new(CommandOptionType::Boolean, "awards-points", "If this world awards points for multiarchi. Defaults to true").required(false))
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::Boolean,
                    "use-claims",
                    "If claiming slots for this world uses the multiarchi claim pool. Defaults to true",
                )
                .required(false),
            )
            .add_option(CreateCommandOption::new(CommandOptionType::String, "message", "Additional message to display").required(false))
    }

    async fn execute(bot: &Bot, ctx: Context, command: CommandInteraction) {
        let user = command.user.id;

        if !bot.admins.contains(&user) {
            command.simple_reply(&ctx, "You do not have permission to use this command").await;
            return;
        }

        let mut tracker = "";
        let mut world_name = "";
        let mut reality_name = None;
        let mut import_claims = "";
        let mut awards_points = true;
        let mut use_claims = true;
        let mut message = None;

        for ResolvedOption { name: option_name, value, .. } in command.data.options() {
            match (option_name, value) {
                ("tracker", ResolvedValue::String(value)) => tracker = value,
                ("name", ResolvedValue::String(value)) => world_name = value,
                ("reality", ResolvedValue::String(value)) => reality_name = Some(value),
                ("import-claims", ResolvedValue::String(value)) => import_claims = value,
                ("awards-points", ResolvedValue::Boolean(value)) => awards_points = value,
                ("use-claims", ResolvedValue::Boolean(value)) => use_claims = value,
                ("message", ResolvedValue::String(value)) => message = Some(value),
                _ => (),
            }
        }

        if tracker.is_empty() {
            command.simple_reply(&ctx, "A tracker link or id is required").await;
            return;
        }

        if world_name.is_empty() {
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

        let _ = command.defer_ephemeral(&ctx.http).await;

        let tracker_id = if let Some(id) = tracker.split('/').next_back() {
            id
        } else {
            let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().content("A tracker url or id is required")).await;
            return;
        };

        let tracker_response = if let Ok(response) = fetch_tracker(tracker_id).await {
            response
        } else {
            let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().content("Failed to fetch tracker")).await;
            return;
        };

        let data = if let Some(data) = scrape(&tracker_response) {
            data
        } else {
            let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().content("Failed to parse tracker")).await;
            return;
        };

        let world_id = if let Ok(response) = query!("INSERT INTO tracked_worlds (tracker_id, name, reality) VALUES (?, ?, ?) RETURNING id", tracker_id, world_name, reality)
            .fetch_one(&bot.db)
            .await
        {
            response.id
        } else {
            let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().content("Failed to create tracked world")).await;
            return;
        };

        let free = if use_claims { 0 } else { 1 };
        let mut unclaimed_slots = 0;
        resolve_preclaims(bot, world_name).await;
        for (slot, data) in data {
            let game_str = game_str(&data.games);
            let points = if awards_points { calc_points(&data.games) } else { 0 };
            let status_i64 = data.status.as_i64();
            let slot_id = if let Ok(response) = query!(
                "INSERT INTO tracked_slots (world, name, games, status, checks, checks_total, last_activity, points, free) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
                world_id,
                slot,
                game_str,
                status_i64,
                data.checks,
                data.checks_total,
                data.last_activity,
                points,
                free
            )
            .fetch_one(&bot.db)
            .await
            {
                response.id
            } else {
                println!("Failed to insert slot {slot} in world {world_id}");
                continue;
            };

            if let Ok(response) = query!(
                "SELECT player FROM preclaims WHERE status = 2 AND slot IN (SELECT id FROM slots WHERE name = ? AND world in (SELECT id FROM worlds WHERE name = ?)) LIMIT 1",
                slot,
                world_name
            )
            .fetch_one(&bot.db)
            .await
            {
                if query!("INSERT INTO claims (slot, player) VALUES (?, ?)", slot_id, response.player).execute(&bot.db).await.is_err() {
                    println!("Failed to transfer preclaim to claim for slot {slot} in world {world_id}");
                }
            } else if let Ok(response) = query!(
                "SELECT player FROM claims WHERE slot IN (SELECT id FROM tracked_slots WHERE name = ? AND world in (SELECT id FROM tracked_worlds WHERE name = ?)) LIMIT 1",
                slot,
                import_claims
            )
            .fetch_one(&bot.db)
            .await
            {
                if query!("INSERT INTO claims (slot, player) VALUES (?, ?)", slot_id, response.player).execute(&bot.db).await.is_err() {
                    println!("Failed to transfer claim for slot {slot} in world {world_id}");
                }
            } else {
                unclaimed_slots += 1;
            }
        }

        bot.push_needed().await;

        let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().content("Started tracking world")).await;

        if unclaimed_slots > 0 {
            if let Some(claims_channel) = Bot::claims_channel(&ctx).await {
                let _ = claims_channel
                    .send_message(
                        &ctx,
                        CreateMessage::new().content(format!(
                            "[<@&1342191138056175707>] New world `{world_name}` available with {unclaimed_slots} unclaimed slots{}. Use `/claim` make your claims.{}",
                            if let Some(reality_name) = reality_name { format!(" in {reality_name}") } else { String::new() },
                            if let Some(message) = message { format!(" {message}") } else { String::new() }
                        )),
                    )
                    .await;
            }
        }
    }

    async fn autocomplete(bot: &Bot, ctx: Context, interaction: CommandInteraction) {
        match interaction.data.autocomplete() {
            Some(AutocompleteOption { name: "name", value, .. }) => bot.autocomplete_preclaim_worlds(ctx, &interaction, value).await,
            Some(AutocompleteOption { name: "reality", value, .. }) => bot.autocomplete_realities(ctx, &interaction, value).await,
            Some(_) | None => {
                interaction.no_autocomplete(&ctx).await;
            }
        }
    }
}

const POINTS_OVERRIDE: phf::Map<&'static str, i64> = phf_map! {
    "Clique" | "Autopelago" | "ArchipIDLE" | "Archipelago" | "APBingo" => 0,
    "Keymaster's Keep" | "Stardew Valley" => 2
};

fn calc_points(games: &[String]) -> i64 {
    1 + games.iter().map(|game| POINTS_OVERRIDE.get(game).copied().unwrap_or(1)).sum::<i64>()
}

fn game_str(games: &[String]) -> String {
    let mut hash = HashMap::new();
    for game in games {
        hash.entry(game).and_modify(|count| *count += 1).or_insert(1);
    }

    hash.into_iter()
        .map(|(game, count)| if count > 1 { format!("{game} x{count}") } else { game.clone() })
        .collect::<Vec<_>>()
        .join(", ")
}
