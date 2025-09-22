use serenity::all::{CommandInteraction, CommandType, Context, CreateCommand, CreateEmbed, EditInteractionResponse};
use sqlx::query;

use crate::{scrape::Status, util::SimpleReply, Bot, Command};

pub struct ClaimedCommand {}

impl Command for ClaimedCommand {
    fn register() -> CreateCommand {
        CreateCommand::new("claimed").description("View your claimed slots").kind(CommandType::ChatInput)
    }

    async fn execute(bot: &Bot, ctx: Context, command: CommandInteraction) {
        let user = i64::from(command.user.id);

        let player = if let Some(player) = bot.get_player(user).await {
            player
        } else {
            command.simple_reply(&ctx, "Failed to get user").await;
            return;
        };

        let _ = command.defer_ephemeral(&ctx.http).await;

        if let Ok(response) = query!("SELECT tracked_worlds.name AS world, tracked_slots.name AS slot, status, free FROM claims INNER JOIN tracked_slots ON claims.slot = tracked_slots.id INNER JOIN tracked_worlds ON tracked_slots.world = tracked_worlds.id WHERE player = ? LIMIT 20", player.id).fetch_all(&bot.db).await {
            let fields = if response.is_empty() {
                vec![(String::from("Overview"), format!("**Used claims**: 0/{}", player.claims), false)]
            } else {
                let mut used_claims = 0;

                let mut game_fields = vec![];
                for record in response {
                    if record.free == 0 && record.status < 2 {
                        used_claims += 1;
                    }

                    if let Some(status) = Status::from_i64(record.status) {
                        game_fields.push((
                            format!("**{}**: {}", record.world, record.slot),
                            format!("**Status**: {}{}", status.as_str(), if record.free > 0 { "\n*Free claim*" } else { "" }),
                            false,
                        ));
                    }
                }

                let mut fields = vec![(String::from("Overview"), format!("**Used claims**: {}/{}", used_claims, player.claims), false)];
                fields.extend(game_fields);

                fields
            };

            let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().embed(CreateEmbed::new().fields(fields))).await;
        } else {
            let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().content("Failed to get unclaimed slots")).await;
        }
    }
}
