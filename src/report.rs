use serenity::all::{
    CommandInteraction, CommandOptionType, CommandType, Context, CreateCommand, CreateCommandOption, CreateEmbed, CreateEmbedFooter, EditInteractionResponse, ResolvedOption, ResolvedValue,
};
use sqlx::query;

use crate::{scrape::Status, util::SimpleReply, Bot, Command};

pub struct ReportCommand {}

impl Command for ReportCommand {
    fn register() -> CreateCommand {
        CreateCommand::new("report")
            .description("Gets a report")
            .kind(CommandType::ChatInput)
            .add_option(CreateCommandOption::new(CommandOptionType::String, "world", "Name of the world").required(true))
            .add_option(CreateCommandOption::new(CommandOptionType::String, "slot", "Name of the slot").required(false))
    }

    async fn execute(bot: &Bot, ctx: Context, command: CommandInteraction) {
        let mut world = "";
        let mut slot = None;

        for ResolvedOption { name: option_name, value, .. } in command.data.options() {
            match (option_name, value) {
                ("world", ResolvedValue::String(value)) => world = value,
                ("slot", ResolvedValue::String(value)) => slot = Some(value),
                _ => (),
            }
        }

        if world.is_empty() {
            command.simple_reply(&ctx, "A world is required").await;
            return;
        }

        let _ = command.defer_ephemeral(&ctx.http).await;

        if bot.admins.contains(&command.user.id) {
            bot.update_scrape(world).await;
        }

        if let Some(slot) = slot {
            if let Ok(slot_response) = query!(
                "SELECT id, games, status, checks, checks_total, last_activity FROM tracked_slots WHERE name = ? AND world IN (SELECT id FROM tracked_worlds WHERE name = ?) LIMIT 1",
                slot,
                world
            )
            .fetch_one(&bot.db)
            .await
            {
                if let Ok(status_response) = query!(
                    "SELECT timestamp, players.snowflake as player_id, description FROM updates INNER JOIN players ON updates.player = players.id WHERE slot = ? ORDER BY timestamp DESC LIMIT 20",
                    slot_response.id
                )
                .fetch_all(&bot.db)
                .await
                {
                    let player_str = if let Ok(response) = query!("SELECT snowflake FROM claims INNER JOIN players ON claims.player = players.id WHERE slot = ?", slot_response.id).fetch_one(&bot.db).await {
                        format!("**Claimed by**: <@{}>", response.snowflake)
                    } else {
                        String::from("*Unclaimed*")
                    };

                    let _ = command
                        .edit_response(
                            &ctx.http,
                            EditInteractionResponse::new().add_embed(
                                CreateEmbed::new()
                                    .title(format!("{world} - {slot}"))
                                    .field(
                                        "Overview",
                                        format!(
                                            "**Games**: {}\n{}\n**Checks**: {}/{}\n**Status**: {}\n**Last activity**: {}",
                                            slot_response.games,
                                            player_str,
                                            slot_response.checks,
                                            slot_response.checks_total,
                                            Status::from_i64(slot_response.status).map(|status| status.as_str()).unwrap_or("Unknown"),
                                            if let Some(minutes) = slot_response.last_activity {
                                                format!("{} hours, {} minutes ago", minutes / 60, minutes % 60)
                                            } else {
                                                String::from("Never")
                                            }
                                        ),
                                        false,
                                    )
                                    .field(
                                        "Updates",
                                        status_response
                                            .into_iter()
                                            .map(|record| format!("[<t:{}:f>] [<@{}>] {}", record.timestamp, record.player_id, record.description))
                                            .collect::<Vec<_>>()
                                            .join("\n"),
                                        false,
                                    ),
                            ),
                        )
                        .await;
                }
            } else {
                let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().content("Failed to get slot info")).await;
            }
        } else if let Ok(slot_response) = query!(
            "SELECT id, name, status, checks, checks_total, last_activity FROM tracked_slots WHERE status < 3 AND world IN (SELECT id FROM tracked_worlds WHERE name = ?) ORDER BY last_activity DESC NULLS FIRST",
            world
        )
        .fetch_all(&bot.db)
        .await
        {
            if slot_response.is_empty() {
                let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().content("World does not exist")).await;
                return;
            }

            let mut all_checks = 0;
            let mut all_checks_total = 0;
            let mut slot_reports = vec![];

            for (i, record) in slot_response.into_iter().enumerate() {
                all_checks += record.checks;
                all_checks_total += record.checks_total;

                if i >= 20 {
                    continue;
                }

                let mut updates = vec![];

                if let Ok(status_response) = query!(
                        "SELECT timestamp, players.snowflake as player_id, description FROM updates INNER JOIN players ON updates.player = players.id WHERE slot = ? GROUP BY updates.player HAVING timestamp = MAX(timestamp) ORDER BY timestamp DESC LIMIT 8",
                        record.id
                    )
                    .fetch_all(&bot.db)
                    .await {
                        for record in status_response {
                            updates.push((record.timestamp, record.player_id, record.description));
                        }
                    }

                let player_str = if let Ok(response) = query!("SELECT snowflake FROM claims INNER JOIN players ON claims.player = players.id WHERE slot = ?", record.id).fetch_one(&bot.db).await {
                    format!("**Claimed by**: <@{}>", response.snowflake)
                } else {
                    String::from("*Unclaimed*")
                };

                slot_reports.push((record.name, player_str, record.checks, record.checks_total, record.status, record.last_activity, updates));
            }

            let _ = command
                .edit_response(
                    &ctx.http,
                    EditInteractionResponse::new().add_embed(
                        CreateEmbed::new()
                            .title(world)
                            .footer(CreateEmbedFooter::new(format!("Total checks: {all_checks}/{all_checks_total}")))
                            .fields(slot_reports.into_iter().map(|(name, player_str, checks, checks_total, status, last_activity, updates)| {
                                (
                                    format!("**{name}** ({checks}/{checks_total})"),
                                    format!(
                                        "{player_str}\n**Status**: {}\n**Last activity**: {}\n{}",
                                        Status::from_i64(status).map(|status| status.as_str()).unwrap_or("Unknown"),
                                        if let Some(minutes) = last_activity {
                                            format!("{} hours, {} minutes ago", minutes / 60, minutes % 60)
                                        } else {
                                            String::from("Never")
                                        },
                                        updates
                                            .into_iter()
                                            .map(|(timestamp, player_id, description)| format!(
                                                "[<t:{}:f>] [{}] {}",
                                                timestamp.unwrap_or_default(),
                                                if let Some(player_id) = player_id { format!("<@{player_id}>") } else { String::from("Unknown") },
                                                description.unwrap_or_default()
                                            ))
                                            .collect::<Vec<_>>()
                                            .join("\n")
                                    ),
                                    false,
                                )
                            })),
                    ),
                )
                .await;
        } else {
            let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().content("Failed to get slot info")).await;
        }
    }
}
