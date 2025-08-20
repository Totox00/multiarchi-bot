#![feature(iter_next_chunk)]
#![feature(iter_array_chunks)]

mod get_preclaims;
mod new_world;
mod util;
mod view_preclaims;

use std::env;

use dotenvy::from_filename_override;
use new_world::NewWorldCommand;
use serenity::{
    all::{Command, Context, EventHandler, GatewayIntents, Interaction, Ready, UserId},
    async_trait, Client,
};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    SqlitePool,
};
use view_preclaims::ViewPreclaimsCommand;

use crate::get_preclaims::GetPreclaimsCommand;

struct Bot {
    db: SqlitePool,
    admins: Vec<UserId>,
}

#[async_trait]
impl EventHandler for Bot {
    async fn ready(&self, ctx: Context, _ready: Ready) {
        if let Err(err) = Command::create_global_command(&ctx.http, ViewPreclaimsCommand::register()).await {
            println!("Failed to create view-preclaims command: {err}");
        }
        if let Err(err) = Command::create_global_command(&ctx.http, NewWorldCommand::register()).await {
            println!("Failed to create new-world command: {err}");
        }
        if let Err(err) = Command::create_global_command(&ctx.http, GetPreclaimsCommand::register()).await {
            println!("Failed to create get-preclaims command: {err}");
        }
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        match interaction {
            Interaction::Command(command) => match command.data.name.as_str() {
                "view-preclaims" => ViewPreclaimsCommand::execute(self, ctx, command).await,
                "new-world" => NewWorldCommand::execute(self, ctx, command).await,
                "get-preclaims" => GetPreclaimsCommand::execute(self, ctx, command).await,
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
    ];

    let bot = Bot { db, admins };

    let intents = GatewayIntents::empty();
    let mut client = Client::builder(&token, intents).event_handler(bot).await.expect("Error creating client");

    if let Err(err) = client.start().await {
        println!("Client error: {err:?}");
    }
}
