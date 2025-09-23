#![feature(iter_next_chunk)]
#![feature(iter_array_chunks)]

mod cancel_preclaims;
mod claim;
mod claimed;
mod done;
mod finish_world;
mod get_preclaims;
mod mark_free;
mod new_world;
mod public;
mod report;
mod reschedule_preclaims;
mod scrape;
mod status;
mod track_world;
mod unclaim;
mod unclaimed;
mod util;
mod view_preclaims;
mod worlds;

use std::env;

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
use new_world::NewWorldCommand;
use rustls::crypto::{aws_lc_rs, CryptoProvider};
use serde_json::json;
use serenity::{
    all::{ChannelId, Command as SerenityCommand, CommandInteraction, Context, CreateCommand, EventHandler, GatewayIntents, Guild, GuildChannel, GuildId, Interaction, Ready, UserId},
    async_trait, Client as DiscordClient,
};
use sqlx::{
    query, query_as,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    SqlitePool,
};
use view_preclaims::ViewPreclaimsCommand;

use crate::{
    cancel_preclaims::CancelPreclaimsCommand, claim::ClaimCommand, claimed::ClaimedCommand, done::DoneCommand, finish_world::FinishWorldCommand, get_preclaims::GetPreclaimsCommand,
    mark_free::MarkFreeCommand, public::PublicCommand, report::ReportCommand, reschedule_preclaims::ReschedulePreclaimsCommand, scrape::Status, status::StatusCommand, track_world::TrackWorldCommand,
    unclaim::UnclaimCommand, unclaimed::UnclaimedCommand, worlds::WorldsCommand,
};

const STATUS_GUILD: u64 = 903349199456841739;
const STATUS_CHANNEL: u64 = 949331929872867348;
const DEFAULT_CLAIMS: i64 = 1;
const SHEET_ID: &str = "10HUN4HG3m9kQZAwZVYSn0k4GkPYCd6lzBeip5Eh1DFU";
const SHEET_RANGE: &str = "data!A1:D";

struct Bot {
    db: SqlitePool,
    admins: Vec<UserId>,
    sheets: Sheets<HttpsConnector<HttpConnector>>,
}

trait Command {
    const NAME: &'static str;

    fn register() -> CreateCommand;
    async fn execute(bot: &Bot, ctx: Context, command: CommandInteraction);
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

    async fn status_channel(ctx: &Context) -> Option<GuildChannel> {
        let guild = Guild::get(ctx, GuildId::new(STATUS_GUILD)).await.ok()?;
        let mut channels = guild.channels(&ctx.http).await.ok()?;
        channels.remove(&ChannelId::new(STATUS_CHANNEL))
    }

    async fn push_to_sheet(&self) {
        let _ = self.sheets.spreadsheets().values_clear(ClearValuesRequest::default(), SHEET_ID, SHEET_RANGE).doit().await;

        let Ok(data) = query!("SELECT world, slot, status, free, player FROM sheets_push").fetch_all(&self.db).await else {
            return;
        };

        let _ = self
            .sheets
            .spreadsheets()
            .values_update(
                ValueRange {
                    major_dimension: Some(String::from("ROWS")),
                    range: Some(String::from("data!A1:D")),
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
                SHEET_RANGE,
            )
            .value_input_option("RAW")
            .doit()
            .await;
    }
}

#[async_trait]
impl EventHandler for Bot {
    async fn ready(&self, ctx: Context, _ready: Ready) {
        register::<ViewPreclaimsCommand>(&ctx).await;
        register::<NewWorldCommand>(&ctx).await;
        register::<GetPreclaimsCommand>(&ctx).await;
        register::<TrackWorldCommand>(&ctx).await;
        register::<ClaimCommand>(&ctx).await;
        register::<StatusCommand>(&ctx).await;
        register::<ReportCommand>(&ctx).await;
        register::<UnclaimCommand>(&ctx).await;
        register::<MarkFreeCommand>(&ctx).await;
        register::<PublicCommand>(&ctx).await;
        register::<UnclaimedCommand>(&ctx).await;
        register::<ClaimedCommand>(&ctx).await;
        register::<FinishWorldCommand>(&ctx).await;
        register::<ReschedulePreclaimsCommand>(&ctx).await;
        register::<CancelPreclaimsCommand>(&ctx).await;
        register::<WorldsCommand>(&ctx).await;
        register::<DoneCommand>(&ctx).await;
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        match interaction {
            Interaction::Command(command) => match command.data.name.as_str() {
                ViewPreclaimsCommand::NAME => ViewPreclaimsCommand::execute(self, ctx, command).await,
                NewWorldCommand::NAME => NewWorldCommand::execute(self, ctx, command).await,
                GetPreclaimsCommand::NAME => GetPreclaimsCommand::execute(self, ctx, command).await,
                TrackWorldCommand::NAME => TrackWorldCommand::execute(self, ctx, command).await,
                ClaimCommand::NAME => ClaimCommand::execute(self, ctx, command).await,
                StatusCommand::NAME => StatusCommand::execute(self, ctx, command).await,
                ReportCommand::NAME => ReportCommand::execute(self, ctx, command).await,
                UnclaimCommand::NAME => UnclaimCommand::execute(self, ctx, command).await,
                MarkFreeCommand::NAME => MarkFreeCommand::execute(self, ctx, command).await,
                PublicCommand::NAME => PublicCommand::execute(self, ctx, command).await,
                UnclaimedCommand::NAME => UnclaimedCommand::execute(self, ctx, command).await,
                ClaimedCommand::NAME => ClaimedCommand::execute(self, ctx, command).await,
                FinishWorldCommand::NAME => FinishWorldCommand::execute(self, ctx, command).await,
                ReschedulePreclaimsCommand::NAME => ReschedulePreclaimsCommand::execute(self, ctx, command).await,
                CancelPreclaimsCommand::NAME => CancelPreclaimsCommand::execute(self, ctx, command).await,
                WorldsCommand::NAME => WorldsCommand::execute(self, ctx, command).await,
                DoneCommand::NAME => DoneCommand::execute(self, ctx, command).await,
                _ => (),
            },
            Interaction::Component(component) => {
                if let Some((_, rest)) = component.data.custom_id.split_once("view-preclaims-") {
                    ViewPreclaimsCommand::handle_interraction(self, ctx, &component, rest).await;
                } else if let Some((_, rest)) = component.data.custom_id.split_once("unclaimed-") {
                    UnclaimedCommand::handle_interraction(self, ctx, &component, rest).await;
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

    let bot = Bot { db, admins, sheets };

    let intents = GatewayIntents::empty();
    let mut client = DiscordClient::builder(&token, intents).event_handler(bot).await.expect("Error creating client");

    if let Err(err) = client.start().await {
        println!("Client error: {err:?}");
    }
}

async fn register<T: Command>(ctx: &Context) {
    if let Err(err) = SerenityCommand::create_global_command(&ctx.http, T::register()).await {
        println!("Failed to create {} command: {err}", T::NAME);
    }
}
