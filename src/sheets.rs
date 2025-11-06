use std::time::{SystemTime, UNIX_EPOCH};

use google_sheets4::api::{ClearValuesRequest, ValueRange};
use http_body_util::BodyExt;
use serde::Deserialize;
use serde_json::{from_slice, json};
use sqlx::query;

use crate::{scrape::Status, Bot};

const SHEET_ID: &str = "1f0lmzxugcrut7q0Y8dSmCzZkfHw__Rwu-z6PCy3j7s4";

#[derive(Debug, Deserialize)]
struct UnspentPointsResponse {
    #[serde(rename = "valueRanges")]
    value_ranges: [UnspentPointsResponseValue; 2],
}

#[derive(Debug, Deserialize)]
struct UnspentPointsResponseValue {
    values: [Vec<String>; 1],
}

impl Bot {
    pub async fn push_needed(&self) {
        if let Some(mut guard) = self.pending_push.lock() {
            *guard = true;
        } else {
            println!("Failed to aquire pending push lock");
            return;
        }

        self.push_to_sheet().await;
    }

    pub async fn push_to_sheet(&self) {
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

    pub async fn update_unspent_points(&self) {
        let response = match self
            .sheets
            .spreadsheets()
            .values_batch_get(SHEET_ID)
            .add_ranges("Archivists!A2:A")
            .add_ranges("Archivists!D2:D")
            .major_dimension("COLUMNS")
            .doit()
            .await
        {
            Ok((response, _)) => response.into_body(),
            Err(err) => {
                dbg!(err);
                return;
            }
        };

        let bytes = match response.collect().await {
            Ok(bytes) => bytes.to_bytes(),
            Err(err) => {
                dbg!(err);
                return;
            }
        };

        let data = match from_slice::<UnspentPointsResponse>(&bytes) {
            Ok(data) => data,
            Err(err) => {
                dbg!(err);
                return;
            }
        };

        for (name, unspent) in data.value_ranges[0].values[0].iter().zip(data.value_ranges[1].values[0].iter()) {
            if let Ok(unspent) = unspent.parse::<i64>() {
                if let Err(err) = query!("UPDATE players SET unspent_points = ? WHERE name = lower(?)", unspent, name).execute(&self.db).await {
                    println!("Failed to set unspent points for {name}: {err:?}");
                }
            } else {
                println!("Failed to parse unspent points: {unspent}");
            }
        }
    }
}
