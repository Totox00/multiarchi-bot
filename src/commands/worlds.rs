use serenity::all::{CommandInteraction, CommandType, Context, CreateCommand, CreateEmbed, CreateInteractionResponseFollowup, EditInteractionResponse};
use sqlx::{query, query_as};

use crate::{commands::Command, Bot};

enum World {
    Tracked(TrackedWorld),
    Preclaim(PreclaimWorld),
}

struct TrackedWorld {
    name: String,
    unclaimed: i64,
    unstarted: i64,
    in_progress: i64,
    goal: i64,
    all_checks: i64,
    done: i64,
}

struct PreclaimWorld {
    name: String,
    slots: i64,
    preclaims: i64,
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

            bot.push_needed().await;
        }

        let mut worlds = vec![];

        if let Ok(response) = query_as!(PreclaimWorld, "SELECT name, slots, preclaims FROM preclaims_overview ORDER BY id").fetch_all(&bot.db).await {
            worlds.extend(response.into_iter().map(World::from));
        } else {
            let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().content("Failed to get preclaims overview")).await;
        }

        if let Ok(response) = query_as!(TrackedWorld, "SELECT name, unclaimed, unstarted, in_progress, goal, all_checks, done FROM worlds_overview ORDER BY id")
            .fetch_all(&bot.db)
            .await
        {
            worlds.extend(response.into_iter().map(World::from));
        } else {
            let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().content("Failed to get worlds overview")).await;
        }

        let mut iter = worlds.into_iter().array_chunks::<25>();
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
    }
}

fn create_embed(chunk: &[World]) -> CreateEmbed {
    CreateEmbed::new().title("Overview").fields(chunk.iter().map(World::field))
}

impl From<PreclaimWorld> for World {
    fn from(value: PreclaimWorld) -> Self {
        Self::Preclaim(value)
    }
}

impl From<TrackedWorld> for World {
    fn from(value: TrackedWorld) -> Self {
        Self::Tracked(value)
    }
}

impl World {
    fn field(&self) -> (&str, String, bool) {
        match self {
            World::Tracked(tracked_world) => {
                let mut entries = vec![];

                if tracked_world.unclaimed > 0 {
                    entries.push(format!("Unclaimed: {}", tracked_world.unclaimed));
                }

                if tracked_world.unstarted > 0 {
                    entries.push(format!("Unstarted: {}", tracked_world.unstarted));
                }

                if tracked_world.in_progress > 0 {
                    entries.push(format!("In progress: {}", tracked_world.in_progress));
                }

                if tracked_world.goal > 0 {
                    entries.push(format!("Goal: {}", tracked_world.goal));
                }

                if tracked_world.all_checks > 0 {
                    entries.push(format!("All checks: {}", tracked_world.all_checks));
                }

                if tracked_world.done > 0 {
                    entries.push(format!("Done: {}", tracked_world.done));
                }

                (&tracked_world.name, entries.join("\n"), false)
            }
            World::Preclaim(preclaim_world) => {
                let mut entries = vec![];

                if preclaim_world.slots > 0 {
                    entries.push(format!("Slots: {}", preclaim_world.slots));
                }

                if preclaim_world.preclaims > 0 {
                    entries.push(format!("Preclaims: {}", preclaim_world.preclaims));
                }

                (&preclaim_world.name, entries.join("\n"), false)
            }
        }
    }
}
