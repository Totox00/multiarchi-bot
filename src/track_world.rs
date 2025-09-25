use serenity::all::{CommandInteraction, CommandOptionType, CommandType, Context, CreateCommand, CreateCommandOption, CreateMessage, EditInteractionResponse, ResolvedOption, ResolvedValue};
use sqlx::query;

use crate::{
    get_preclaims::resolve_preclaims,
    scrape::{fetch_tracker, scrape},
    util::SimpleReply,
    Bot, Command,
};

pub struct TrackWorldCommand {}

impl Command for TrackWorldCommand {
    const NAME: &'static str = "track-world";

    fn register() -> CreateCommand {
        CreateCommand::new(Self::NAME)
            .description("Tracks a new world")
            .kind(CommandType::ChatInput)
            .add_option(CreateCommandOption::new(CommandOptionType::String, "tracker", "Link to or id of the tracker for this world").required(true))
            .add_option(CreateCommandOption::new(CommandOptionType::String, "name", "Name of the world").required(true))
    }

    async fn execute(bot: &Bot, ctx: Context, command: CommandInteraction) {
        let user = command.user.id;

        if !bot.admins.contains(&user) {
            command.simple_reply(&ctx, "You do not have permission to use this command").await;
            return;
        }

        let mut tracker = "";
        let mut world_name = "";

        for ResolvedOption { name: option_name, value, .. } in command.data.options() {
            match (option_name, value) {
                ("tracker", ResolvedValue::String(value)) => tracker = value,
                ("name", ResolvedValue::String(value)) => world_name = value,
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

        let world_id = if let Ok(response) = query!("INSERT INTO tracked_worlds (tracker_id, name) VALUES (?, ?) RETURNING id", tracker_id, world_name)
            .fetch_one(&bot.db)
            .await
        {
            response.id
        } else {
            let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().content("Failed to create tracked world")).await;
            return;
        };

        resolve_preclaims(bot, world_name).await;
        for (slot, data) in data {
            let game_str = data.games.join(", ");
            let status_i64 = data.status.as_i64();
            let slot_id = if let Ok(response) = query!(
                "INSERT INTO tracked_slots (world, name, games, status, checks, checks_total, last_activity) VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id",
                world_id,
                slot,
                game_str,
                status_i64,
                data.checks,
                data.checks_total,
                data.last_activity
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
            }
        }

        let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().content("Started tracking world")).await;

        if let Some(claims_channel) = Bot::claims_channel(&ctx).await {
            let _ = claims_channel
                .send_message(&ctx, CreateMessage::new().content("[TEST] New world available. Use `/claim` make your claims."))
                .await;
        }
    }
}
