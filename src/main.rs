#![feature(iter_next_chunk)]
#![feature(iter_array_chunks)]

mod claim;
mod get_preclaims;
mod mark_free;
mod new_world;
mod public;
mod report;
mod scrape;
mod status;
mod track_world;
mod unclaim;
mod unclaimed;
mod util;
mod view_preclaims;

use std::env;

use dotenvy::from_filename_override;
use new_world::NewWorldCommand;
use serenity::{
    all::{Command as SerenityCommand, CommandInteraction, Context, CreateCommand, EventHandler, GatewayIntents, Interaction, Ready, UserId},
    async_trait, Client,
};
use sqlx::{
    query,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    SqlitePool,
};
use view_preclaims::ViewPreclaimsCommand;

use crate::{
    claim::ClaimCommand, get_preclaims::GetPreclaimsCommand, mark_free::MarkFreeCommand, public::PublicCommand, report::ReportCommand, status::StatusCommand, track_world::TrackWorldCommand,
    unclaim::UnclaimCommand, unclaimed::UnclaimedCommand,
};

const DEFAULT_CLAIMS: i64 = 1;

struct Bot {
    db: SqlitePool,
    admins: Vec<UserId>,
}

trait Command {
    fn register() -> CreateCommand;
    async fn execute(bot: &Bot, ctx: Context, command: CommandInteraction);
}

struct Player {
    pub id: i64,
    pub claims: i64,
}

impl Bot {
    async fn get_player(&self, snowflake: i64) -> Option<Player> {
        if let Ok(response) = query!("SELECT id, claims FROM players WHERE snowflake = ? LIMIT 1", snowflake).fetch_one(&self.db).await {
            Some(Player {
                id: response.id,
                claims: response.claims,
            })
        } else {
            let response = query!("INSERT INTO players (snowflake, claims) VALUES (?, ?) RETURNING id", snowflake, DEFAULT_CLAIMS)
                .fetch_one(&self.db)
                .await
                .ok()?;
            Some(Player {
                id: response.id,
                claims: DEFAULT_CLAIMS,
            })
        }
    }
}

#[async_trait]
impl EventHandler for Bot {
    async fn ready(&self, ctx: Context, _ready: Ready) {
        register::<ViewPreclaimsCommand>("view-preclaims", &ctx).await;
        register::<NewWorldCommand>("new-world", &ctx).await;
        register::<GetPreclaimsCommand>("get-preclaims", &ctx).await;
        register::<TrackWorldCommand>("track-world", &ctx).await;
        register::<ClaimCommand>("claim", &ctx).await;
        register::<StatusCommand>("status", &ctx).await;
        register::<ReportCommand>("report", &ctx).await;
        register::<UnclaimCommand>("unclaim", &ctx).await;
        register::<MarkFreeCommand>("mark-free", &ctx).await;
        register::<PublicCommand>("public", &ctx).await;
        register::<UnclaimedCommand>("unclaimed", &ctx).await;
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        match interaction {
            Interaction::Command(command) => match command.data.name.as_str() {
                "view-preclaims" => ViewPreclaimsCommand::execute(self, ctx, command).await,
                "new-world" => NewWorldCommand::execute(self, ctx, command).await,
                "get-preclaims" => GetPreclaimsCommand::execute(self, ctx, command).await,
                "track-world" => TrackWorldCommand::execute(self, ctx, command).await,
                "claim" => ClaimCommand::execute(self, ctx, command).await,
                "status" => StatusCommand::execute(self, ctx, command).await,
                "report" => ReportCommand::execute(self, ctx, command).await,
                "unclaim" => UnclaimCommand::execute(self, ctx, command).await,
                "mark-free" => MarkFreeCommand::execute(self, ctx, command).await,
                "public" => PublicCommand::execute(self, ctx, command).await,
                "unclaimed" => UnclaimedCommand::execute(self, ctx, command).await,
                _ => (),
            },
            Interaction::Component(component) => {
                if let Some((_, rest)) = component.data.custom_id.split_once("view-preclaims-") {
                    ViewPreclaimsCommand::handle_interraction(self, ctx, &component, rest).await;
                }
            }
            _ => (),
        }
    }
}

#[tokio::main]
async fn main() {
    from_filename_override(".env").expect("Failed to load .env file");

    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");

    let db = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(SqliteConnectOptions::new().filename("db.sqlite").create_if_missing(true))
        .await
        .expect("Couldn't connect to database");

    let admins = vec![
        UserId::new(458684324653301770), // totox00
        UserId::new(622847469495123990), // dragorrod
        UserId::new(764623297932820501), // elirefeltores (tester)
    ];

    let bot = Bot { db, admins };

    let intents = GatewayIntents::empty();
    let mut client = Client::builder(&token, intents).event_handler(bot).await.expect("Error creating client");

    if let Err(err) = client.start().await {
        println!("Client error: {err:?}");
    }
}

async fn register<T: Command>(name: &str, ctx: &Context) {
    if let Err(err) = SerenityCommand::create_global_command(&ctx.http, T::register()).await {
        println!("Failed to create {name} command: {err}");
    }
}
