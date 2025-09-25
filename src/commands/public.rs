use std::collections::HashMap;

use serenity::all::{
    AutocompleteOption, CommandInteraction, CommandOptionType, CommandType, Context, CreateCommand, CreateCommandOption, CreateEmbed, EditInteractionResponse, ResolvedOption, ResolvedValue,
};
use sqlx::query;

use crate::{autocomplete::Autocomplete, commands::Command, Bot};

pub struct PublicCommand {}

impl Command for PublicCommand {
    const NAME: &'static str = "public";

    fn register() -> CreateCommand {
        CreateCommand::new(Self::NAME)
            .description("View public slots or mark a slot of yours as public")
            .kind(CommandType::ChatInput)
            .add_option(CreateCommandOption::new(CommandOptionType::String, "world", "Name of the world").required(false).set_autocomplete(true))
            .add_option(CreateCommandOption::new(CommandOptionType::String, "slot", "Name of the slot").required(false).set_autocomplete(true))
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::String,
                    "description",
                    "A description, recommended to include game and specify which games in the slot are public",
                )
                .required(false),
            )
    }

    async fn execute(bot: &Bot, ctx: Context, command: CommandInteraction) {
        let mut world = "";
        let mut slot = "";
        let mut description = "";

        for ResolvedOption { name: option_name, value, .. } in command.data.options() {
            match (option_name, value) {
                ("world", ResolvedValue::String(value)) => world = value,
                ("slot", ResolvedValue::String(value)) => slot = value,
                ("description", ResolvedValue::String(value)) => description = value,
                _ => (),
            }
        }

        let _ = command.defer_ephemeral(&ctx.http).await;

        if world.is_empty() {
            if let Ok(response) = query!("SELECT world, slot, description FROM public_claims").fetch_all(&bot.db).await {
                if response.is_empty() {
                    let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().content("No worlds have public claims")).await;
                } else {
                    let mut worlds: HashMap<String, Vec<(String, String)>> = HashMap::new();

                    for record in response {
                        if let Some(description) = record.description {
                            worlds
                                .entry(record.world)
                                .and_modify(|entry| entry.push((record.slot.to_owned(), description.to_owned())))
                                .or_insert(vec![(record.slot, description)]);
                        }
                    }

                    let _ = command
                        .edit_response(
                            &ctx.http,
                            EditInteractionResponse::new().embed(CreateEmbed::new().fields(worlds.into_iter().map(|(world, slots)| {
                                (
                                    world,
                                    slots.into_iter().map(|(name, description)| format!("**{name}**: {description}")).collect::<Vec<_>>().join("\n"),
                                    false,
                                )
                            }))),
                        )
                        .await;
                }
            } else {
                let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().content("Failed to get public claims")).await;
            }

            return;
        }

        if slot.is_empty() {
            if let Ok(response) = query!("SELECT slot, description FROM public_claims WHERE world = ?", world).fetch_all(&bot.db).await {
                if response.is_empty() {
                    let _ = command
                        .edit_response(&ctx.http, EditInteractionResponse::new().content(format!("{world} does not have public claims")))
                        .await;
                } else {
                    let _ = command
                        .edit_response(
                            &ctx.http,
                            EditInteractionResponse::new().embed(
                                CreateEmbed::new().field(
                                    world,
                                    response
                                        .into_iter()
                                        .filter_map(|record| record.description.map(|description| format!("**{}**: {description}", record.slot)))
                                        .collect::<Vec<_>>()
                                        .join("\n"),
                                    false,
                                ),
                            ),
                        )
                        .await;
                }
            } else {
                let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().content("Failed to get public claims")).await;
            }

            return;
        }

        if description.is_empty() {
            description = "*No description provided*";
        }

        let user = i64::from(command.user.id);
        if let Ok(response) = query!(
            "UPDATE claims SET public = ? WHERE player IN (SELECT id FROM players WHERE snowflake = ?) AND slot IN (SELECT id FROM tracked_slots WHERE name = ? AND world IN (SELECT id FROM tracked_worlds WHERE name = ?))",
            description,
            user,
            slot,
            world
        )
        .execute(&bot.db)
        .await
        {
            if response.rows_affected() == 0 {
                let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().content("Failed to mark claim as public")).await;
            } else {
                let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().content("Successfully marked claim as public")).await;
            }
        } else {
            let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().content("Failed to mark claim as public")).await;
        }
    }

    async fn autocomplete(bot: &Bot, ctx: Context, interaction: CommandInteraction) {
        match interaction.data.autocomplete() {
            Some(AutocompleteOption { name: "world", value, .. }) => bot.autocomplete_worlds(ctx, &interaction, value).await,
            Some(AutocompleteOption { name: "slot", value, .. }) => {
                let mut world = None;
                for ResolvedOption { name: option_name, value, .. } in interaction.data.options() {
                    if let ("world", ResolvedValue::String(value)) = (option_name, value) {
                        world = Some(value)
                    }
                }

                bot.autocomplete_slots(ctx, &interaction, value, world).await;
            }
            Some(_) | None => {
                interaction.no_autocomplete(&ctx).await;
            }
        }
    }
}
