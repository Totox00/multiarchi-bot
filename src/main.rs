#![feature(iter_next_chunk)]
#![feature(iter_array_chunks)]

mod autocomplete;
mod channels;
mod commands;
mod paginate;
mod scrape;
mod sheets;
mod util;

use std::{
    env,
    fs::{File, OpenOptions},
    io::Write,
    sync::Arc,
    time::Duration,
};

use chrono::Local;
use dotenvy::from_filename_override;
use google_sheets4::{
    hyper_util::{
        client::legacy::{connect::HttpConnector, Client as SheetsClient},
        rt::TokioExecutor,
    },
    yup_oauth2::{read_service_account_key, ServiceAccountAuthenticator},
    Sheets,
};
use http_body_util::combinators::BoxBody;
use hyper_rustls::{HttpsConnector, HttpsConnectorBuilder};
use rustls::{
    crypto::{aws_lc_rs, CryptoProvider},
    lock::Mutex,
};
use serenity::{
    all::{Context, EventHandler, GatewayIntents, Interaction, Ready, UserId},
    async_trait, Client as DiscordClient,
};
use sqlx::{
    query, query_as,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    SqlitePool,
};
use tokio::{spawn, time::interval};

use crate::commands::interaction_create;

const MAX_REALITIES: usize = 2;
const NO_REALITY_CLAIMS: i64 = 2;
const LOG_PATH: &str = "bot.log";

struct Bot {
    db: SqlitePool,
    admins: Vec<UserId>,
    sheets: Sheets<HttpsConnector<HttpConnector>>,
    latest_push: Arc<Mutex<u64>>,
    pending_push: Arc<Mutex<bool>>,
    log: Arc<Mutex<File>>,
}

struct Player {
    pub id: i64,
    pub name: String,
    pub points: i64,
}

#[derive(Clone, Copy)]
struct Reality {
    pub id: i64,
    pub max_claims: i64,
}

impl Bot {
    async fn get_player(&self, snowflake: i64, name: &str) -> Option<Player> {
        if let Ok(response) = query_as!(Player, "SELECT id, name, points FROM players WHERE snowflake = ? LIMIT 1", snowflake)
            .fetch_one(&self.db)
            .await
        {
            Some(response)
        } else {
            let response = query!("INSERT INTO players (snowflake, name) VALUES (?, ?) RETURNING id, points", snowflake, name)
                .fetch_one(&self.db)
                .await
                .ok()?;
            Some(Player {
                id: response.id,
                name: name.to_owned(),
                points: response.points,
            })
        }
    }

    fn log(&self, content: &str) {
        let timestamp = Local::now();

        if let Some(mut guard) = self.log.lock() {
            if writeln!(guard, "[{timestamp}] {content}").is_err() {
                println!("[FAILED LOG WRITE] [{timestamp}] {content}");
            }
        } else {
            println!("[FAILED TO AQUIRE LOG FILE LOCK] [{timestamp}] {content}");
        }
    }

    async fn can_claim_slot(&self, player: i64, slot: i64) -> Result<(), &'static str> {
        if let Ok(response) = query!(
            "SELECT realities.id, realities.max_claims FROM tracked_slots LEFT JOIN tracked_worlds ON tracked_worlds.id = tracked_slots.world LEFT JOIN realities ON realities.id = tracked_worlds.reality WHERE tracked_slots.id = ?",
            slot
        )
        .fetch_optional(&self.db)
        .await
        {
            self.can_claim_in_reality(
                player,
                response.map(|record| Reality {
                    id: record.id,
                    max_claims: record.max_claims.unwrap_or(NO_REALITY_CLAIMS),
                }),
            )
            .await
        } else {
            Err("Failed to get reality")
        }
    }

    async fn can_preclaim_slot(&self, player: i64, slot: i64) -> Result<(), &'static str> {
        if let Ok(response) = query!(
            "SELECT realities.id, realities.max_claims FROM slots LEFT JOIN worlds ON worlds.id = slots.world LEFT JOIN realities ON realities.id = worlds.reality WHERE slots.id = ?",
            slot
        )
        .fetch_optional(&self.db)
        .await
        {
            self.can_claim_in_reality(
                player,
                response.map(|record| Reality {
                    id: record.id,
                    max_claims: record.max_claims.unwrap_or(NO_REALITY_CLAIMS),
                }),
            )
            .await
        } else {
            Err("Failed to get reality")
        }
    }

    async fn can_claim_in_reality(&self, player: i64, reality: Option<Reality>) -> Result<(), &'static str> {
        let (max_claims, current_claims) = if let Some(reality) = reality {
            let realities: Vec<_> = if let Ok(response) = query!("SELECT reality FROM current_realities WHERE player = ?", player).fetch_all(&self.db).await {
                response.into_iter().filter_map(|record| record.reality).collect()
            } else {
                return Err("Failed to get current realities");
            };

            if !realities.contains(&reality.id) && realities.len() >= MAX_REALITIES {
                return Err("You cannot join more realities");
            }

            (
                reality.max_claims,
                if let Ok(response) = query!("SELECT claims FROM current_claims WHERE player = ? AND reality = ?", player, reality.id)
                    .fetch_optional(&self.db)
                    .await
                {
                    response.map(|record| record.claims).unwrap_or(0)
                } else {
                    return Err("Failed to get current claims");
                },
            )
        } else {
            (
                NO_REALITY_CLAIMS,
                if let Ok(response) = query!("SELECT claims FROM current_claims WHERE player = ? AND reality IS NULL", player).fetch_optional(&self.db).await {
                    response.map(|record| record.claims).unwrap_or(0)
                } else {
                    return Err("Failed to get current claims");
                },
            )
        };

        if current_claims >= max_claims {
            return Err("No available claim");
        }

        Ok(())
    }
}

#[async_trait]
impl EventHandler for &Bot {
    async fn ready(&self, _ctx: Context, _ready: Ready) {}

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        interaction_create(self, ctx, interaction).await;
    }
}

#[tokio::main]
async fn main() {
    from_filename_override(".env").expect("Failed to load .env file");

    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");

    let log = OpenOptions::new().append(true).create(true).open(LOG_PATH).unwrap();

    let db = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(SqliteConnectOptions::new().filename("db.sqlite").create_if_missing(true))
        .await
        .expect("Couldn't connect to database");

    let admins = vec![
        UserId::new(458684324653301770), // totox00
        UserId::new(622847469495123990), // dragorrod
    ];

    let _ = CryptoProvider::install_default(aws_lc_rs::default_provider());
    let client: SheetsClient<_, BoxBody<google_sheets4::hyper::body::Bytes, google_sheets4::hyper::Error>> = SheetsClient::builder(TokioExecutor::new()).build(
        HttpsConnectorBuilder::new()
            .with_native_roots()
            .expect("Failed to set tls config")
            .https_only()
            .enable_http1()
            .enable_http2()
            .build(),
    );
    let secret = read_service_account_key("sheets_key.json").await.expect("Failed to find sheets api key file");
    let auth = ServiceAccountAuthenticator::with_client(
        secret,
        SheetsClient::builder(TokioExecutor::new()).build(
            HttpsConnectorBuilder::new()
                .with_native_roots()
                .expect("Failed to set tls config")
                .https_only()
                .enable_http1()
                .enable_http2()
                .build(),
        ),
    )
    .build()
    .await
    .expect("Failed to create an authenticator");

    let sheets = Sheets::new(client, auth);

    let bot = Box::new(Bot {
        db,
        admins,
        sheets,
        latest_push: Arc::new(Mutex::new(0)),
        pending_push: Arc::new(Mutex::new(false)),
        log: Arc::new(Mutex::new(log)),
    });

    let bot: &'static Bot = Box::leak(bot);

    spawn(async {
        let mut interval = interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            bot.push_to_sheet().await;
        }
    });

    let intents = GatewayIntents::empty();
    let mut client = DiscordClient::builder(&token, intents).event_handler(bot).await.expect("Error creating client");

    if let Err(err) = client.start().await {
        println!("Client error: {err:?}");
    }
}
