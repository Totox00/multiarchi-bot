use serenity::all::{
    AutocompleteOption, CommandInteraction, CommandOptionType, CommandType, Context, CreateCommand, CreateCommandOption, CreateEmbed, CreateMessage, EditInteractionResponse, ResolvedOption,
    ResolvedValue,
};
use sqlx::query;

use crate::{autocomplete::Autocomplete, commands::Command, util::SimpleReply, Bot};

pub struct FinishWorldCommand {}

impl Command for FinishWorldCommand {
    const NAME: &'static str = "finish-world";

    fn register() -> CreateCommand {
        CreateCommand::new(Self::NAME)
            .description("Awards points for a world and deletes it and all claims in it")
            .kind(CommandType::ChatInput)
            .add_option(CreateCommandOption::new(CommandOptionType::String, "world", "Name of the world").required(true).set_autocomplete(true))
    }

    async fn execute(bot: &Bot, ctx: Context, command: CommandInteraction) {
        let mut world = "";

        for ResolvedOption { name: option_name, value, .. } in command.data.options() {
            if let ("world", ResolvedValue::String(value)) = (option_name, value) {
                world = value
            }
        }

        if world.is_empty() {
            command.simple_reply(&ctx, "A world is required").await;
            return;
        }

        if !bot.admins.contains(&command.user.id) {
            command.simple_reply(&ctx, "You do not have permission to use this command").await;
            return;
        }

        let Ok(mut transaction) = bot.db.begin().await else {
            command.simple_reply(&ctx, "Failed to create transaction").await;
            return;
        };

        let _ = command.defer_ephemeral(&ctx.http).await;

        let mut output = vec![];

        if let Err(err) = query!(
            "DELETE FROM updates WHERE slot IN (SELECT id FROM tracked_slots WHERE world IN (SELECT id FROM tracked_worlds WHERE name = ?))",
            world
        )
        .execute(&mut *transaction)
        .await
        {
            println!("Failed to delete status updates: {err}");
            let _ = transaction.rollback().await;
            let _ = command
                .edit_response(&ctx.http, EditInteractionResponse::new().content("Failed to delete status updates. Aborting"))
                .await;
            return;
        }

        if let Ok(slot_response) = query!("SELECT id, name, games, points FROM tracked_slots WHERE world in (SELECT id FROM tracked_worlds WHERE name = ?)", world)
            .fetch_all(&mut *transaction)
            .await
        {
            for record in slot_response {
                match query!(
                    "UPDATE players SET points = points + ? WHERE id IN (SELECT player FROM claims WHERE slot = ?) RETURNING snowflake",
                    record.points,
                    record.id
                )
                .fetch_optional(&mut *transaction)
                .await
                {
                    Ok(response) => {
                        if let Some(response) = response {
                            output.push((record.name, Some(response.snowflake)));

                            if let Err(err) = query!("DELETE FROM claims WHERE slot = ?", record.id).execute(&mut *transaction).await {
                                println!("Failed to delete claim: {err}");
                                let _ = transaction.rollback().await;
                                let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().content("Failed to delete claim. Aborting")).await;
                                return;
                            }
                        } else {
                            output.push((record.name, None));
                        }

                        if let Err(err) = query!("DELETE FROM tracked_slots WHERE id = ?", record.id).execute(&mut *transaction).await {
                            println!("Failed to delete slot: {err}");
                            let _ = transaction.rollback().await;
                            let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().content("Failed to delete slot. Aborting")).await;
                            return;
                        }
                    }
                    Err(err) => {
                        println!("Failed to update points: {err}");
                        let _ = transaction.rollback().await;
                        let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().content("Failed to update points. Aborting")).await;
                        return;
                    }
                }
            }
        } else {
            let _ = transaction.rollback().await;
            let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().content("Failed to get slots")).await;
            return;
        }

        if let Err(err) = query!("DELETE FROM tracked_worlds WHERE name = ?", world).execute(&mut *transaction).await {
            println!("Failed to delete world: {err}");
            let _ = transaction.rollback().await;
            let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().content("Failed to delete world. Aborting")).await;
            return;
        }

        if let Err(err) = query!("DELETE FROM worlds WHERE name = ?", world).execute(&mut *transaction).await {
            println!("Failed to delete preclaim world: {err}");
            let _ = transaction.rollback().await;
            let _ = command
                .edit_response(&ctx.http, EditInteractionResponse::new().content("Failed to delete preclaim world. Aborting"))
                .await;
            return;
        }

        let Some(status_channel) = Bot::status_channel(&ctx).await else {
            let _ = transaction.rollback().await;
            let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().content("Failed to get status channel. Aborting")).await;
            return;
        };

        let mut iter = output.into_iter().array_chunks::<50>();
        for chunk in iter.by_ref() {
            if let Err(err) = status_channel
                .send_message(
                    &ctx,
                    CreateMessage::new().embed(
                        CreateEmbed::new().title(format!("{world} completed!")).description(
                            chunk
                                .into_iter()
                                .map(|(slot, player)| format!("**{slot}** [{}]", if let Some(player) = player { format!("<@{player}>") } else { String::from("*Unclaimed*") }))
                                .collect::<Vec<_>>()
                                .join("\n"),
                        ),
                    ),
                )
                .await
            {
                println!("Failed to post completion to status channel: {err}");
                let _ = transaction.rollback().await;
                let _ = command
                    .edit_response(&ctx.http, EditInteractionResponse::new().content("Failed to post completion to status channel. Aborting"))
                    .await;
                return;
            }
        }

        if let Some(chunk) = iter.into_remainder() {
            let chunk: Vec<_> = chunk.collect();
            if !chunk.is_empty() {
                if let Err(err) = status_channel
                    .send_message(
                        &ctx,
                        CreateMessage::new().embed(
                            CreateEmbed::new().title(format!("{world} completed!")).description(
                                chunk
                                    .into_iter()
                                    .map(|(slot, player)| format!("**{slot}** [{}]", if let Some(player) = player { format!("<@{player}>") } else { String::from("*Unclaimed*") }))
                                    .collect::<Vec<_>>()
                                    .join("\n"),
                            ),
                        ),
                    )
                    .await
                {
                    println!("Failed to post completion to status channel: {err}");
                    let _ = transaction.rollback().await;
                    let _ = command
                        .edit_response(&ctx.http, EditInteractionResponse::new().content("Failed to post completion to status channel. Aborting"))
                        .await;
                    return;
                }
            }
        }

        if let Err(err) = transaction.commit().await {
            println!("Failed to commit transaction: {err}");
            let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().content("Failed to commit transaction. Aborting")).await;
            return;
        }

        bot.push_needed().await;
        let _ = command
            .edit_response(&ctx.http, EditInteractionResponse::new().content(format!("Successfully finished world {world}")))
            .await;
    }

    async fn autocomplete(bot: &Bot, ctx: Context, interaction: CommandInteraction) {
        match interaction.data.autocomplete() {
            Some(AutocompleteOption { name: "world", value, .. }) => bot.autocomplete_worlds(ctx, &interaction, value).await,
            Some(_) | None => {
                interaction.no_autocomplete(&ctx).await;
            }
        }
    }
}
