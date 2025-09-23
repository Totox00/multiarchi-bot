use serenity::all::{CommandInteraction, CommandType, Context, CreateCommand, CreateEmbed, CreateInteractionResponseFollowup, EditInteractionResponse};
use sqlx::{query, query_as};

use crate::{Bot, Command};

struct World {
    name: String,
    unclaimed: i64,
    in_progress: i64,
    goal: i64,
    all_checks: i64,
    done: i64,
}

pub struct WorldsCommand {}

impl Command for WorldsCommand {
    const NAME: &'static str = "worlds";

    fn register() -> CreateCommand {
        CreateCommand::new(Self::NAME).description("Gets an overview of all current worlds").kind(CommandType::ChatInput)
    }

    async fn execute(bot: &Bot, ctx: Context, command: CommandInteraction) {
        let _ = command.defer_ephemeral(&ctx.http).await;

        if bot.admins.contains(&command.user.id) {
            if let Ok(response) = query!("SELECT name FROM tracked_worlds WHERE id IN (SELECT world FROM tracked_slots WHERE status < 2)")
                .fetch_all(&bot.db)
                .await
            {
                for record in response {
                    bot.update_scrape(&record.name).await;
                }
            }
        }

        if let Ok(response) = query_as!(World, "SELECT name, unclaimed, in_progress, goal, all_checks, done FROM worlds_overview ORDER BY id")
            .fetch_all(&bot.db)
            .await
        {
            let mut iter = response.into_iter().array_chunks::<25>();
            let mut edited = false;

            if let Some(chunk) = iter.next() {
                let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().add_embed(create_embed(&chunk))).await;
                edited = true;
                for chunk in iter.by_ref() {
                    let _ = command
                        .create_followup(&ctx.http, CreateInteractionResponseFollowup::new().ephemeral(true).add_embed(create_embed(&chunk)))
                        .await;
                }
            }

            if let Some(chunk) = iter.into_remainder() {
                if edited {
                    let _ = command
                        .create_followup(&ctx.http, CreateInteractionResponseFollowup::new().ephemeral(true).add_embed(create_embed(&chunk.collect::<Vec<_>>())))
                        .await;
                } else {
                    let _ = command
                        .edit_response(&ctx.http, EditInteractionResponse::new().add_embed(create_embed(&chunk.collect::<Vec<_>>())))
                        .await;
                }
            }
        } else {
            let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().content("Failed to get overview")).await;
        }
    }
}

fn create_embed(chunk: &[World]) -> CreateEmbed {
    CreateEmbed::new().title("Overview").fields(chunk.iter().map(|world| {
        let mut entries = vec![];

        if world.unclaimed > 0 {
            entries.push(format!("Unclaimed: {}", world.unclaimed));
        }

        if world.in_progress > 0 {
            entries.push(format!("In progress: {}", world.in_progress));
        }

        if world.goal > 0 {
            entries.push(format!("Goal: {}", world.goal));
        }

        if world.all_checks > 0 {
            entries.push(format!("All checks: {}", world.all_checks));
        }

        if world.done > 0 {
            entries.push(format!("Done: {}", world.done));
        }

        (&world.name, entries.join("\n"), false)
    }))
}
