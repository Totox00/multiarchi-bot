use phf::phf_map;
use serenity::all::{
    CommandInteraction, CommandOptionType, CommandType, Context, CreateCommand, CreateCommandOption, CreateEmbed, CreateMessage, EditInteractionResponse, ResolvedOption, ResolvedValue,
};
use sqlx::query;

use crate::{util::SimpleReply, Bot, Command};

pub struct FinishWorldCommand {}

impl Command for FinishWorldCommand {
    const NAME: &'static str = "finish-world";

    fn register() -> CreateCommand {
        CreateCommand::new(Self::NAME)
            .description("Awards points for a world and deletes it and all claims in it")
            .kind(CommandType::ChatInput)
            .add_option(CreateCommandOption::new(CommandOptionType::String, "world", "Name of the world").required(true))
    }

    async fn execute(bot: &Bot, ctx: Context, command: CommandInteraction) {
        let mut world = "";

        for ResolvedOption { name: option_name, value, .. } in command.data.options() {
            if let ("world", ResolvedValue::String(value)) = (option_name, value) {
                world = value
            }
        }

        if world.is_empty() {
            command.simple_reply(&ctx, "A world is required").await;
            return;
        }

        if !bot.admins.contains(&command.user.id) {
            command.simple_reply(&ctx, "You do not have permission to use this command").await;
            return;
        }

        let Ok(mut transaction) = bot.db.begin().await else {
            command.simple_reply(&ctx, "Failed to create transaction").await;
            return;
        };

        let _ = command.defer_ephemeral(&ctx.http).await;

        let mut output = vec![];

        if let Ok(slot_response) = query!("SELECT id, name, games FROM tracked_slots WHERE world in (SELECT id FROM tracked_worlds WHERE name = ?)", world)
            .fetch_all(&mut *transaction)
            .await
        {
            for record in slot_response {
                let new_points = calc_points(&record.games);
                if let Ok(response) = query!(
                    "UPDATE players SET points = points + ? WHERE id IN (SELECT player FROM claims WHERE slot = ?) RETURNING snowflake, points",
                    new_points,
                    record.id
                )
                .fetch_optional(&mut *transaction)
                .await
                {
                    if let Some(response) = response {
                        // handle change from new points

                        output.push((record.name, Some(response.snowflake)));

                        if query!("DELETE FROM claims WHERE slot = ?", record.id).execute(&mut *transaction).await.is_err() {
                            let _ = transaction.rollback().await;
                            let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().content("Failed to delete claim. Aborting")).await;
                            return;
                        }

                        if query!("DELETE FROM tracked_slots WHERE id = ?", record.id).execute(&mut *transaction).await.is_err() {
                            let _ = transaction.rollback().await;
                            let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().content("Failed to delete slot. Aborting")).await;
                            return;
                        }
                    } else {
                        output.push((record.name, None));

                        if query!("DELETE FROM tracked_slots WHERE id = ?", record.id).execute(&mut *transaction).await.is_err() {
                            let _ = transaction.rollback().await;
                            let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().content("Failed to delete slot. Aborting")).await;
                            return;
                        }
                    }
                } else {
                    let _ = transaction.rollback().await;
                    let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().content("Failed to update points. Aborting")).await;
                    return;
                }
            }
        } else {
            let _ = transaction.rollback().await;
            let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().content("Failed to get slots")).await;
            return;
        }

        if query!("DELETE FROM tracked_worlds WHERE name = ?", world).execute(&mut *transaction).await.is_err() {
            let _ = transaction.rollback().await;
            let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().content("Failed to delete world. Aborting")).await;
            return;
        }

        let Some(status_channel) = Bot::status_channel(&ctx).await else {
            let _ = transaction.rollback().await;
            let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().content("Failed to get status channel. Aborting")).await;
            return;
        };

        let mut iter = output.into_iter().array_chunks::<50>();
        for chunk in iter.by_ref() {
            if status_channel
                .send_message(
                    &ctx,
                    CreateMessage::new().embed(
                        CreateEmbed::new().title(format!("{world} completed!")).description(
                            chunk
                                .into_iter()
                                .map(|(slot, player)| format!("**{slot}** [{}]", if let Some(player) = player { format!("<@{player}>") } else { String::from("*Unclaimed*") }))
                                .collect::<Vec<_>>()
                                .join("\n"),
                        ),
                    ),
                )
                .await
                .is_err()
            {
                let _ = transaction.rollback().await;
                let _ = command
                    .edit_response(&ctx.http, EditInteractionResponse::new().content("Failed to post completion to status channel. Aborting"))
                    .await;
                return;
            }
        }

        if let Some(chunk) = iter.into_remainder() {
            let chunk: Vec<_> = chunk.collect();
            if !chunk.is_empty()
                && status_channel
                    .send_message(
                        &ctx,
                        CreateMessage::new().embed(
                            CreateEmbed::new().title(format!("{world} completed!")).description(
                                chunk
                                    .into_iter()
                                    .map(|(slot, player)| format!("**{slot}** [{}]", if let Some(player) = player { format!("<@{player}>") } else { String::from("*Unclaimed*") }))
                                    .collect::<Vec<_>>()
                                    .join("\n"),
                            ),
                        ),
                    )
                    .await
                    .is_err()
            {
                let _ = transaction.rollback().await;
                let _ = command
                    .edit_response(&ctx.http, EditInteractionResponse::new().content("Failed to post completion to status channel. Aborting"))
                    .await;
                return;
            }
        }

        if transaction.commit().await.is_err() {
            let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().content("Failed to commit transaction. Aborting")).await;
            return;
        }

        let _ = command
            .edit_response(&ctx.http, EditInteractionResponse::new().content(format!("Successfully finished world {world}")))
            .await;
    }
}

const POINTS_OVERRIDE: phf::Map<&'static str, i64> = phf_map! {
    "Clique" | "Autopelago" | "ArchipIDLE" | "Archipelago" | "APBingo" => 0,
    "Keymaster's Keep" | "Stardew Valley" => 2
};

fn calc_points(games: &str) -> i64 {
    1 + games.split(", ").map(|game| POINTS_OVERRIDE.get(game).copied().unwrap_or(1)).sum::<i64>()
}
