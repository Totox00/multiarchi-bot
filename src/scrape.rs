use std::collections::HashMap;

use scraper::{Html, Selector};
use sqlx::query;

use crate::Bot;

pub struct SlotData {
    pub status: Status,
    pub games: Vec<String>,
    pub checks: u32,
    pub checks_total: u32,
    pub last_activity: Option<u32>,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Status {
    Unstarted,
    InProgress,
    Goal,
    AllChecks,
    Done,
}

pub async fn fetch_tracker(id: &str) -> Result<String, reqwest::Error> {
    reqwest::get(format!("https://archipelago.gg/tracker/{id}")).await?.text().await
}

impl Bot {
    pub async fn update_scrape(&self, world: &str) {
        let (id, tracker_id) = if let Ok(response) = query!(
            "SELECT id, tracker_id FROM tracked_worlds WHERE name = ? AND last_scrape < (strftime('%s', 'now') - 3600) LIMIT 1",
            world
        )
        .fetch_one(&self.db)
        .await
        {
            if let Some(id) = response.id {
                (id, response.tracker_id)
            } else {
                return;
            }
        } else {
            return;
        };

        if query!("UPDATE tracked_worlds SET last_scrape = (strftime('%s', 'now')) WHERE id = ?", id)
            .execute(&self.db)
            .await
            .is_err()
        {
            println!("Failed to update last_scrape for world {world}");
        }

        let html = if let Ok(html) = fetch_tracker(&tracker_id).await {
            html
        } else {
            return;
        };

        let data = if let Some(data) = scrape(&html) {
            data
        } else {
            return;
        };

        let mut all_goal = true;

        for (slot, data) in data {
            if all_goal && data.status != Status::Goal && data.status != Status::Done {
                all_goal = false;
            }

            let status_i64 = data.status.as_i64();
            if query!(
                "UPDATE tracked_slots SET status = ?, checks = ?, last_activity = ? WHERE name = ?",
                status_i64,
                data.checks,
                data.last_activity,
                slot
            )
            .execute(&self.db)
            .await
            .is_err()
            {
                println!("Failed to update data for slot {slot} in world {world}");
            }
        }

        if all_goal && query!("UPDATE tracked_worlds SET done = 1 WHERE id = ?", id).execute(&self.db).await.is_err() {
            println!("Failed to mark world {world} as done");
        }
    }
}

pub fn scrape(html: &str) -> Option<HashMap<String, SlotData>> {
    let document = Html::parse_document(html);
    let slot_table_selector = Selector::parse("#checks-table > tbody").ok()?;
    let slots = document.select(&slot_table_selector).next()?;

    let mut out = HashMap::new();

    for slot in slots.child_elements() {
        let mut iter = slot.child_elements();
        iter.next();
        let mut name = iter.next()?.text().next()?;
        let game = iter.next()?.text().next()?;
        let mut status = if iter.next()?.text().next()?.contains("Goal Completed") {
            Status::Goal
        } else {
            Status::InProgress
        };
        let (checks, checks_total) = iter.next()?.text().next()?.split_once('/')?;
        iter.next();
        let last_activity = iter.next()?.text().next()?;

        if name.ends_with(')') {
            name = name.split('(').next_back()?.trim_end_matches(')');
        }
        name = name.trim_end_matches(['1', '2', '3', '4', '5', '6', '7', '8', '9', '0']);
        let checks: u32 = checks.trim().parse().ok()?;
        let checks_total: u32 = checks_total.trim().parse().ok()?;

        let last_activity = if last_activity == "None" {
            None
        } else {
            let (seconds, _) = last_activity.split_once('.')?;
            Some(seconds.parse::<u32>().ok()? / 60)
        };

        if status != Status::Goal && checks == 0 {
            status = Status::Unstarted;
        } else if checks == checks_total {
            if status == Status::Goal {
                status = Status::Done;
            } else {
                status = Status::AllChecks;
            }
        }

        out.entry(name.to_string())
            .and_modify(|slot_data: &mut SlotData| {
                slot_data.status.merge(status);
                slot_data.games.push(game.to_string());
                slot_data.checks += checks;
                slot_data.checks_total += checks_total;
                merge_last_activity(&mut slot_data.last_activity, last_activity);
            })
            .or_insert(SlotData {
                status,
                games: vec![game.to_string()],
                checks,
                checks_total,
                last_activity,
            });
    }

    Some(out)
}

impl Status {
    pub fn from_i64(i: i64) -> Option<Status> {
        match i {
            0 => Some(Self::Unstarted),
            1 => Some(Self::InProgress),
            2 => Some(Self::Goal),
            3 => Some(Self::AllChecks),
            4 => Some(Self::Done),
            _ => None,
        }
    }

    fn merge(&mut self, next_status: Status) {
        if next_status == Status::Unstarted {
            *self = Status::Unstarted;
        } else if next_status != Status::Done && ((*self == Status::AllChecks && next_status != Status::AllChecks) || (*self == Status::Goal && next_status != Status::Goal)) {
            *self = Status::InProgress;
        } else if *self == Status::Done {
            *self = next_status;
        }
    }

    pub fn as_i64(&self) -> i64 {
        match self {
            Status::Unstarted => 0,
            Status::InProgress => 1,
            Status::Goal => 2,
            Status::AllChecks => 3,
            Status::Done => 4,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Status::Unstarted => "Unstarted",
            Status::InProgress => "In Progress",
            Status::Goal => "Goal",
            Status::AllChecks => "All Checks",
            Status::Done => "Done",
        }
    }
}

fn merge_last_activity(last_activity: &mut Option<u32>, next_last_activity: Option<u32>) {
    if let Some(next_last_activity) = next_last_activity {
        if let Some(last_activity) = last_activity {
            if next_last_activity > *last_activity {
                *last_activity = next_last_activity;
            }
        }
    } else {
        *last_activity = None;
    }
}
