#![feature(iter_next_chunk)]
#![feature(iter_array_chunks)]

mod autocomplete;
mod channels;
mod commands;
mod scrape;
mod util;

use std::{
    env,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use dotenvy::from_filename_override;
use google_sheets4::{
    api::{ClearValuesRequest, ValueRange},
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
use serde_json::json;
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

use crate::{
    commands::{interaction_create, register_all},
    scrape::Status,
};

const DEFAULT_CLAIMS: i64 = 2;
const SHEET_ID: &str = "1f0lmzxugcrut7q0Y8dSmCzZkfHw__Rwu-z6PCy3j7s4";

struct Bot {
    db: SqlitePool,
    admins: Vec<UserId>,
    sheets: Sheets<HttpsConnector<HttpConnector>>,
    latest_push: Arc<Mutex<u64>>,
    pending_push: Arc<Mutex<bool>>,
}

struct Player {
    pub id: i64,
    pub name: String,
    pub claims: i64,
    pub points: i64,
}

impl Bot {
    async fn get_player(&self, snowflake: i64, name: &str) -> Option<Player> {
        if let Ok(response) = query_as!(Player, "SELECT id, name, claims, points FROM players WHERE snowflake = ? LIMIT 1", snowflake)
            .fetch_one(&self.db)
            .await
        {
            Some(response)
        } else {
            let response = query!("INSERT INTO players (snowflake, name, claims) VALUES (?, ?, ?) RETURNING id, points", snowflake, name, DEFAULT_CLAIMS)
                .fetch_one(&self.db)
                .await
                .ok()?;
            Some(Player {
                id: response.id,
                name: name.to_owned(),
                claims: DEFAULT_CLAIMS,
                points: response.points,
            })
        }
    }

    async fn push_needed(&self) {
        if let Some(mut guard) = self.pending_push.lock() {
            *guard = true;
        } else {
            println!("Failed to aquire pending push lock");
            return;
        }

        self.push_to_sheet().await;
    }

    async fn push_to_sheet(&self) {
        if let (Some(mut latest_guard), Some(mut pending_guard)) = (self.latest_push.lock(), self.pending_push.lock()) {
            if !*pending_guard {
                return;
            }

            let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
            let current_secs = current_time.as_secs();

            if current_secs - *latest_guard < 60 {
                return;
            }

            *latest_guard = current_secs;
            *pending_guard = false;
        } else {
            println!("Failed to aquire sheets locks");
            return;
        }

        let _ = self.sheets.spreadsheets().values_clear(ClearValuesRequest::default(), SHEET_ID, "autodata!A1:G").doit().await;

        let Ok(data) = query!("SELECT world, slot, status, free, player FROM sheets_push").fetch_all(&self.db).await else {
            return;
        };

        let _ = self
            .sheets
            .spreadsheets()
            .values_update(
                ValueRange {
                    major_dimension: Some(String::from("ROWS")),
                    range: Some(String::from("autodata!A1:D")),
                    values: Some(
                        data.into_iter()
                            .filter_map(|record| {
                                Status::from_i64(record.status).map(|status| {
                                    vec![
                                        json!(record.world),
                                        json!(record.slot),
                                        json!(if record.player.is_none() {
                                            if record.free > 0 {
                                                "Unclaimed [Free claim]"
                                            } else {
                                                "Unclaimed"
                                            }
                                        } else {
                                            status.as_str()
                                        }),
                                        json!(record.player),
                                    ]
                                })
                            })
                            .collect(),
                    ),
                },
                SHEET_ID,
                "autodata!A1:D",
            )
            .value_input_option("RAW")
            .doit()
            .await;

        let Ok(data) = query!("SELECT name, points FROM players ORDER BY id").fetch_all(&self.db).await else {
            return;
        };

        let _ = self
            .sheets
            .spreadsheets()
            .values_update(
                ValueRange {
                    major_dimension: Some(String::from("ROWS")),
                    range: Some(String::from("autodata!F1:G")),
                    values: Some(data.into_iter().map(|record| vec![json!(record.name), json!(record.points)]).collect()),
                },
                SHEET_ID,
                "autodata!F1:G",
            )
            .value_input_option("RAW")
            .doit()
            .await;
    }
}

#[async_trait]
impl EventHandler for &Bot {
    async fn ready(&self, ctx: Context, _ready: Ready) {
        register_all(&ctx).await;
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        interaction_create(self, ctx, interaction).await;
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
