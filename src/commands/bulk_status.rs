use serenity::all::{
    AutocompleteOption, CommandInteraction, CommandOptionType, CommandType, Context, CreateCommand, CreateCommandOption, CreateMessage, EditInteractionResponse, ResolvedOption, ResolvedValue,
};
use sqlx::query;

use crate::{autocomplete::Autocomplete, commands::Command, util::SimpleReply, Bot};

pub struct BulkStatusCommand {}

impl Command for BulkStatusCommand {
    const NAME: &'static str = "bulk-status";

    fn register() -> CreateCommand {
        CreateCommand::new(Self::NAME)
            .description("Report the status of multiple slots")
            .kind(CommandType::ChatInput)
            .add_option(CreateCommandOption::new(CommandOptionType::String, "world1", "Name of the world").required(true).set_autocomplete(true))
            .add_option(CreateCommandOption::new(CommandOptionType::String, "slot1", "Name of the slot").required(true).set_autocomplete(true))
            .add_option(CreateCommandOption::new(CommandOptionType::String, "description1", "Status description").required(true))
            .add_option(
                CreateCommandOption::new(CommandOptionType::String, "world2", "Name of the world")
                    .required(false)
                    .set_autocomplete(true),
            )
            .add_option(CreateCommandOption::new(CommandOptionType::String, "slot2", "Name of the slot").required(false).set_autocomplete(true))
            .add_option(CreateCommandOption::new(CommandOptionType::String, "description2", "Status description").required(false))
            .add_option(
                CreateCommandOption::new(CommandOptionType::String, "world3", "Name of the world")
                    .required(false)
                    .set_autocomplete(true),
            )
            .add_option(CreateCommandOption::new(CommandOptionType::String, "slot3", "Name of the slot").required(false).set_autocomplete(true))
            .add_option(CreateCommandOption::new(CommandOptionType::String, "description3", "Status description").required(false))
            .add_option(
                CreateCommandOption::new(CommandOptionType::String, "world4", "Name of the world")
                    .required(false)
                    .set_autocomplete(true),
            )
            .add_option(CreateCommandOption::new(CommandOptionType::String, "slot4", "Name of the slot").required(false).set_autocomplete(true))
            .add_option(CreateCommandOption::new(CommandOptionType::String, "description4", "Status description").required(false))
            .add_option(
                CreateCommandOption::new(CommandOptionType::String, "world5", "Name of the world")
                    .required(false)
                    .set_autocomplete(true),
            )
            .add_option(CreateCommandOption::new(CommandOptionType::String, "slot5", "Name of the slot").required(false).set_autocomplete(true))
            .add_option(CreateCommandOption::new(CommandOptionType::String, "description5", "Status description").required(false))
            .add_option(
                CreateCommandOption::new(CommandOptionType::String, "world6", "Name of the world")
                    .required(false)
                    .set_autocomplete(true),
            )
            .add_option(CreateCommandOption::new(CommandOptionType::String, "slot6", "Name of the slot").required(false).set_autocomplete(true))
            .add_option(CreateCommandOption::new(CommandOptionType::String, "description6", "Status description").required(false))
            .add_option(
                CreateCommandOption::new(CommandOptionType::String, "world7", "Name of the world")
                    .required(false)
                    .set_autocomplete(true),
            )
            .add_option(CreateCommandOption::new(CommandOptionType::String, "slot7", "Name of the slot").required(false).set_autocomplete(true))
            .add_option(CreateCommandOption::new(CommandOptionType::String, "description7", "Status description").required(false))
            .add_option(
                CreateCommandOption::new(CommandOptionType::String, "world8", "Name of the world")
                    .required(false)
                    .set_autocomplete(true),
            )
            .add_option(CreateCommandOption::new(CommandOptionType::String, "slot8", "Name of the slot").required(false).set_autocomplete(true))
            .add_option(CreateCommandOption::new(CommandOptionType::String, "description8", "Status description").required(false))
    }

    async fn execute(bot: &Bot, ctx: Context, command: CommandInteraction) {
        let mut worlds: [Option<&str>; 8] = [None; 8];
        let mut slots: [Option<&str>; 8] = [None; 8];
        let mut descriptions: [Option<&str>; 8] = [None; 8];
        let mut active_world = None;

        for ResolvedOption { name: option_name, value, .. } in command.data.options() {
            match (option_name.split_at(option_name.len() - 1), value) {
                (("world", i), ResolvedValue::String(value)) => {
                    if let Ok(i) = i.parse::<usize>() {
                        worlds[i] = Some(value);
                    }
                }
                (("slot", i), ResolvedValue::String(value)) => {
                    if let Ok(i) = i.parse::<usize>() {
                        slots[i] = Some(value);
                    }
                }
                (("description", i), ResolvedValue::String(value)) => {
                    if let Ok(i) = i.parse::<usize>() {
                        descriptions[i] = Some(value);
                    }
                }
                _ => (),
            }
        }

        let Some(player) = bot.get_player(i64::from(command.user.id), &command.user.name).await else {
            command.simple_reply(&ctx, "Failed to get user").await;
            return;
        };

        let _ = command.defer_ephemeral(&ctx.http).await;
        let mut status_channel_msg = vec![];

        for i in 0..8 {
            if worlds[i].is_some() {
                active_world = worlds[i];
            }
            if let (Some(world), Some(slot), Some(description)) = (active_world, slots[i], descriptions[i]) {
                let slot_id = if let Ok(response) = query!("SELECT id FROM tracked_slots WHERE name = ? AND world in (SELECT id FROM tracked_worlds WHERE name = ?)", slot, world)
                    .fetch_one(&bot.db)
                    .await
                {
                    response.id
                } else {
                    let _ = command
                        .edit_response(&ctx.http, EditInteractionResponse::new().content(format!("Failed to get slot {slot} in {world}")))
                        .await;
                    return;
                };

                if query!("INSERT INTO updates (slot, player, description) VALUES (?, ?, ?)", slot_id, player.id, description)
                    .execute(&bot.db)
                    .await
                    .is_ok()
                {
                    status_channel_msg.push(format!("[{}] [{world}] [{slot}] {description}", command.user.display_name()));
                } else {
                    let _ = command
                        .edit_response(&ctx.http, EditInteractionResponse::new().content(format!("Failed to update status for slot {slot} in {world}")))
                        .await;
                    return;
                }
            }
        }

        if let Some(status_channel) = Bot::status_channel(&ctx).await {
            let _ = status_channel.send_message(&ctx, CreateMessage::new().content(status_channel_msg.join("\n"))).await;
        }

        let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().content("Successfully updated status of slots")).await;
    }

    async fn autocomplete(bot: &Bot, ctx: Context, interaction: CommandInteraction) {
        match interaction.data.autocomplete() {
            Some(AutocompleteOption { name, value, .. }) => {
                if name.starts_with("world") {
                    bot.autocomplete_worlds(ctx, &interaction, value).await;
                } else if let ("slot", slot_i) = name.split_at(4) {
                    let mut world = None;
                    if let Ok(slot_i) = slot_i.parse::<u8>() {
                        let mut world_i = 0;
                        for ResolvedOption { name, value, .. } in interaction.data.options() {
                            if let (name, ResolvedValue::String(value)) = (name, value) {
                                if let ("world", i) = name.split_at(5) {
                                    if let Ok(i) = i.parse::<u8>() {
                                        if slot_i >= i && world_i < i {
                                            world = Some(value);
                                            world_i = i;
                                        }
                                    }
                                }
                            }
                        }
                    };

                    if let Some(player) = bot.get_player(i64::from(interaction.user.id), &interaction.user.name).await {
                        bot.autocomplete_slots_claimed(ctx, &interaction, value, world, &player).await;
                    } else {
                        bot.autocomplete_slots(ctx, &interaction, value, world).await;
                    }
                } else {
                    interaction.no_autocomplete(&ctx).await;
                }
            }
            None => {
                interaction.no_autocomplete(&ctx).await;
            }
        }
    }
}
