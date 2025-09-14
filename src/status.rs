use serenity::all::{
    ChannelId, CommandInteraction, CommandOptionType, CommandType, Context, CreateCommand, CreateCommandOption, CreateMessage, EditInteractionResponse, Guild, GuildId, ResolvedOption, ResolvedValue,
};
use sqlx::query;

use crate::{util::SimpleReply, Bot, Command};

const STATUS_GUILD: u64 = 903349199456841739;
const STATUS_CHANNEL: u64 = 949331929872867348;

pub struct StatusCommand {}

impl Command for StatusCommand {
    fn register() -> CreateCommand {
        CreateCommand::new("status")
            .description("Report the status of a slot")
            .kind(CommandType::ChatInput)
            .add_option(CreateCommandOption::new(CommandOptionType::String, "world", "Name of the world").required(true))
            .add_option(CreateCommandOption::new(CommandOptionType::String, "slot", "Name of the slot").required(true))
            .add_option(CreateCommandOption::new(CommandOptionType::String, "description", "Status description").required(true))
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

        let slot_id = if let Ok(response) = query!("SELECT id FROM tracked_slots WHERE name = ? AND world in (SELECT id FROM tracked_worlds WHERE name = ?)", slot, world)
            .fetch_one(&bot.db)
            .await
        {
            response.id
        } else {
            command.simple_reply(&ctx, "Failed to get slot").await;
            return;
        };

        let player = if let Some(player) = bot.get_player(i64::from(command.user.id)).await {
            player
        } else {
            command.simple_reply(&ctx, "Failed to get user").await;
            return;
        };

        let _ = command.defer_ephemeral(&ctx.http).await;

        if query!("INSERT INTO updates (slot, player, description) VALUES (?, ?, ?)", slot_id, player.id, description)
            .execute(&bot.db)
            .await
            .is_ok()
        {
            if let Ok(guild) = Guild::get(&ctx, GuildId::new(STATUS_GUILD)).await {
                if let Ok(channels) = guild.channels(&ctx.http).await {
                    if let Some(status_channel) = channels.get(&ChannelId::new(STATUS_CHANNEL)) {
                        let _ = status_channel
                            .send_message(&ctx, CreateMessage::new().content(format!("[{}] [{world}] [{slot}] {description}", command.user.display_name())))
                            .await;
                    }
                }
            }

            let _ = command
                .edit_response(&ctx.http, EditInteractionResponse::new().content(format!("Successfully updated status of {slot} in {world}")))
                .await;
        } else {
            let _ = command.edit_response(&ctx.http, EditInteractionResponse::new().content("Failed to update status")).await;
        }
    }
}
