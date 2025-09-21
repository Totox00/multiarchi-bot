use std::collections::HashMap;

use serenity::all::{CommandInteraction, CommandOptionType, CommandType, Context, CreateCommand, CreateCommandOption, CreateEmbed, EditInteractionResponse, ResolvedOption, ResolvedValue};
use sqlx::query;

use crate::{Bot, Command};

pub struct UnclaimedCommand {}

impl Command for UnclaimedCommand {
    fn register() -> CreateCommand {
        CreateCommand::new("unclaimed")
            .description("View unclaimed slots")
            .kind(CommandType::ChatInput)
            .add_option(CreateCommandOption::new(CommandOptionType::String, "world", "Name of the world").required(false))
    }

    async fn execute(bot: &Bot, ctx: Context, command: CommandInteraction) {
        let mut world = "";

        for ResolvedOption { name: option_name, value, .. } in command.data.options() {
            if let ("world", ResolvedValue::String(value)) = (option_name, value) {
                world = value
            }
        }

        let _ = command.defer_ephemeral(&ctx.http).await;

        if world.is_empty() {
            if let Ok(response) = query!("SELECT world, slot, games, free FROM unclaimed_slots LIMIT 100").fetch_all(&bot.db).await {
                if response.is_empty() {
                    let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().content("No worlds have unclaimed slots")).await;
                } else {
                    let mut worlds: HashMap<String, Vec<(String, String, bool)>> = HashMap::new();
                    let mut added = 0;

                    for record in response {
                        if let Some(entry) = worlds.get_mut(&record.world) {
                            entry.push((record.slot, record.games, record.free > 0));
                        } else if added < 20 {
                            worlds.insert(record.world, vec![(record.slot, record.games, record.free > 0)]);
                            added += 1;
                        }
                    }

                    let mut fields = vec![];

                    for (world, slots) in worlds {
                        let mut iter = slots.into_iter().array_chunks::<10>();

                        for chunk in iter.by_ref() {
                            if fields.len() < 25 {
                                fields.push((
                                    world.clone(),
                                    chunk
                                        .into_iter()
                                        .map(|(slot, games, free)| format!("**{slot}**: {games}{}", if free { " [Free Claim]" } else { "" }))
                                        .collect::<Vec<_>>()
                                        .join("\n"),
                                    false,
                                ));
                            }
                        }

                        if fields.len() < 25 {
                            if let Some(chunk) = iter.into_remainder() {
                                let chunk: Vec<_> = chunk.collect();
                                if !chunk.is_empty() {
                                    fields.push((
                                        world.clone(),
                                        chunk
                                            .into_iter()
                                            .map(|(slot, games, free)| format!("**{slot}**: {games}{}", if free { " [Free Claim]" } else { "" }))
                                            .collect::<Vec<_>>()
                                            .join("\n"),
                                        false,
                                    ));
                                }
                            }
                        }
                    }

                    let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().embed(CreateEmbed::new().fields(fields))).await;
                }
            } else {
                let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().content("Failed to get unclaimed slots")).await;
            }

            return;
        }

        if let Ok(response) = query!("SELECT slot, games, free FROM unclaimed_slots WHERE world = ? LIMIT 100", world).fetch_all(&bot.db).await {
            if response.is_empty() {
                let _ = command
                    .edit_response(&ctx.http, EditInteractionResponse::new().content(format!("{world} does not have unclaimed slots")))
                    .await;
            } else {
                let mut fields = vec![];
                let mut iter = response.into_iter().array_chunks::<10>();

                for chunk in iter.by_ref() {
                    fields.push((
                        world,
                        chunk
                            .into_iter()
                            .map(|record| format!("**{}**: {}{}", record.slot, record.games, if record.free > 0 { " [Free Claim]" } else { "" }))
                            .collect::<Vec<_>>()
                            .join("\n"),
                        false,
                    ));
                }

                if let Some(chunk) = iter.into_remainder() {
                    let chunk: Vec<_> = chunk.collect();
                    if !chunk.is_empty() {
                        fields.push((
                            world,
                            chunk
                                .into_iter()
                                .map(|record| format!("**{}**: {}{}", record.slot, record.games, if record.free > 0 { " [Free Claim]" } else { "" }))
                                .collect::<Vec<_>>()
                                .join("\n"),
                            false,
                        ));
                    }
                }

                let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().embed(CreateEmbed::new().fields(fields))).await;
            }
        } else {
            let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().content("Failed to get unclaimed slots")).await;
        }
    }
}
