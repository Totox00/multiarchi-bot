use std::collections::HashMap;

use serenity::all::{
    ButtonStyle, Colour, CommandInteraction, CommandType, ComponentInteraction, ComponentInteractionDataKind, Context, CreateActionRow, CreateButton, CreateCommand, CreateEmbed, CreateEmbedFooter,
    CreateInteractionResponse, CreateInteractionResponseMessage, CreateSelectMenu, CreateSelectMenuKind, CreateSelectMenuOption, Timestamp,
};
use sqlx::query;

use crate::{
    commands::Command,
    util::{get_page, SimpleReply},
    Bot,
};

struct World {
    name: String,
    preclaim_end: i64,
    slots: Vec<i64>,
}

struct Slot {
    id: i64,
    name: String,
    games: String,
    notes: String,
    points: String,
    current_preclaim: bool,
}

pub struct ViewPreclaimsCommand {}

impl Command for ViewPreclaimsCommand {
    const NAME: &'static str = "view-preclaims";

    fn register() -> CreateCommand {
        CreateCommand::new(Self::NAME).description("View worlds with possible preclaims").kind(CommandType::ChatInput)
    }

    async fn execute(bot: &Bot, ctx: Context, command: CommandInteraction) {
        let user_id = i64::from(command.user.id);
        let Some(player) = bot.get_player(user_id, &command.user.name).await else {
            command.simple_reply(&ctx, "Failed to get player").await;
            return;
        };

        let Some(worlds) = get_worlds(bot).await else {
            command.simple_reply(&ctx, "Failed to get current worlds").await;
            return;
        };

        let Some(first_world) = worlds.first() else {
            command.simple_reply(&ctx, "There are no worlds currently accepting preclaims").await;
            return;
        };

        let page_count = worlds.iter().map(|world| if world.slots.len() <= 24 { 1 } else { world.slots.len().div_ceil(20) }).sum();

        let current_preclaim = query!("SELECT name FROM slots INNER JOIN preclaims ON preclaims.slot = slots.id WHERE status = 0 AND player = ?", player.id)
            .fetch_one(&bot.db)
            .await
            .map(|record| record.name)
            .ok();

        let _ = command
            .create_response(
                &ctx.http,
                CreateInteractionResponse::Message(build_embed(
                    first_world,
                    get_slot_details(bot, if first_world.slots.len() <= 24 { &first_world.slots[..] } else { &first_world.slots[0..20] }, player.id).await,
                    0,
                    page_count,
                    current_preclaim,
                )),
            )
            .await;
    }
}

impl ViewPreclaimsCommand {
    pub async fn handle_interraction(bot: &Bot, ctx: Context, interaction: &ComponentInteraction, id: &str) {
        if id == "select" {
            let user_id = i64::from(interaction.user.id);

            let slot_id = if let ComponentInteractionDataKind::StringSelect { values } = &interaction.data.kind {
                if let Some(slot_id) = values.first() {
                    if let Ok(slot_id) = slot_id.parse::<i64>() {
                        slot_id
                    } else {
                        interaction.simple_reply(&ctx, "Malformed slot id").await;
                        return;
                    }
                } else {
                    interaction.simple_reply(&ctx, "Preclaims must be of exactly one slot").await;
                    return;
                }
            } else {
                interaction.simple_reply(&ctx, "Malformed selection data").await;
                return;
            };

            if let Some(player) = bot.get_player(user_id, &interaction.user.name).await {
                if let Ok(response) = query!("SELECT claims FROM current_claims WHERE player = ?", player.id).fetch_optional(&bot.db).await {
                    if response.is_some_and(|record| record.claims > player.claims) {
                        interaction.simple_reply(&ctx, "Cannot preclaim without an available claim").await;
                        return;
                    }
                } else {
                    interaction.simple_reply(&ctx, "Failed to get current claims").await;
                    return;
                }

                if query!("DELETE FROM preclaims WHERE player = ? AND status = 0", player.id).execute(&bot.db).await.is_err() {
                    interaction.simple_reply(&ctx, "Failed to remove old preclaims").await;
                    return;
                }

                if query!("INSERT INTO preclaims (slot, player) VALUES (?, ?)", slot_id, player.id).execute(&bot.db).await.is_err() {
                    interaction.simple_reply(&ctx, "Failed to create preclaim").await;
                    return;
                }
            } else {
                interaction.simple_reply(&ctx, "Failed to get player").await;
                return;
            }

            interaction.simple_reply(&ctx, "Successfully preclaimed slot").await;
        } else if let Some((_, rest)) = id.split_once("page-") {
            if let Ok(new_page) = rest.parse::<usize>() {
                let user_id = i64::from(interaction.user.id);
                let Some(player) = bot.get_player(user_id, &interaction.user.name).await else {
                    interaction.simple_reply(&ctx, "Failed to get player").await;
                    return;
                };

                let worlds = if let Some(worlds) = get_worlds(bot).await {
                    worlds
                } else {
                    interaction.simple_reply(&ctx, "Failed to get current worlds").await;
                    return;
                };

                let page_count = worlds.iter().map(|world| if world.slots.len() <= 24 { 1 } else { world.slots.len().div_ceil(20) }).sum();

                if new_page >= page_count {
                    interaction.simple_reply(&ctx, "Invalid page").await;
                    return;
                }

                let (world, slots) = if let Some((world, slots)) = get_page(&worlds, |world| &world.slots, new_page) {
                    (world, get_slot_details(bot, slots, player.id).await)
                } else {
                    interaction.simple_reply(&ctx, "Invalid page").await;
                    return;
                };

                let current_preclaim = query!("SELECT name FROM slots INNER JOIN preclaims ON preclaims.slot = slots.id WHERE status = 0 AND player = ?", player.id)
                    .fetch_one(&bot.db)
                    .await
                    .map(|record| record.name)
                    .ok();

                let _ = interaction
                    .create_response(&ctx.http, CreateInteractionResponse::UpdateMessage(build_embed(world, slots, new_page, page_count, current_preclaim)))
                    .await;
            } else {
                interaction.simple_reply(&ctx, "Malformed page number").await;
            }
        } else {
            interaction.simple_reply(&ctx, "Unrecognized interraction").await;
        }
    }
}

async fn get_worlds(bot: &Bot) -> Option<Vec<World>> {
    if let Ok(response) = query!(
        "SELECT worlds.name AS world_name, preclaim_end, slots.id AS slot_id FROM worlds INNER JOIN slots ON worlds.id = slots.world WHERE preclaim_end > strftime('%s', 'now') ORDER BY slots.name"
    )
    .fetch_all(&bot.db)
    .await
    {
        let mut worlds: HashMap<String, (i64, Vec<i64>)> = HashMap::new();

        for record in response {
            worlds
                .entry(record.world_name)
                .and_modify(|(_, vec)| vec.push(record.slot_id))
                .or_insert((record.preclaim_end, vec![record.slot_id]));
        }

        let mut worlds_vec: Vec<_> = worlds
            .into_iter()
            .map(|(world_name, (preclaim_end, slots))| World {
                name: world_name,
                preclaim_end,
                slots,
            })
            .collect();
        worlds_vec.sort_by_key(|world| world.preclaim_end);
        Some(worlds_vec)
    } else {
        None
    }
}

async fn get_slot_details(bot: &Bot, slots: &[i64], player: i64) -> Vec<Slot> {
    let mut out = vec![];

    for slot in slots {
        if let Ok(response) = query!("SELECT name, games, notes, points FROM slots WHERE id = ? LIMIT 1", slot).fetch_one(&bot.db).await {
            let current_preclaim = query!("SELECT status FROM preclaims WHERE slot = ? AND player = ? LIMIT 1", slot, player)
                .fetch_one(&bot.db)
                .await
                .is_ok();

            out.push(Slot {
                id: *slot,
                name: response.name,
                games: response.games,
                notes: response.notes,
                points: response.points,
                current_preclaim,
            });
        }
    }

    out
}

fn build_embed(world: &World, slots: Vec<Slot>, page: usize, page_count: usize, current_preclaim: Option<String>) -> CreateInteractionResponseMessage {
    let mut components = vec![];

    if page_count > 1 {
        components.push(CreateActionRow::Buttons(vec![
            CreateButton::new(format!("view-preclaims-page-{}", page - 1))
                .disabled(page == 0)
                .style(ButtonStyle::Secondary)
                .label("← Prev"),
            CreateButton::new("view-preclaims-page-count")
                .disabled(true)
                .style(ButtonStyle::Secondary)
                .label(format!("Page {} of {page_count}", page + 1)),
            CreateButton::new(format!("view-preclaims-page-{}", page + 1))
                .disabled(page + 1 == page_count)
                .style(ButtonStyle::Secondary)
                .label("Next →"),
        ]));
    }

    components.push(CreateActionRow::SelectMenu(
        CreateSelectMenu::new(
            "view-preclaims-select",
            CreateSelectMenuKind::String {
                options: slots.iter().map(|slot| CreateSelectMenuOption::new(&slot.name, slot.id.to_string())).collect(),
            },
        )
        .placeholder("Make preclaim")
        .min_values(1)
        .max_values(1),
    ));

    CreateInteractionResponseMessage::new()
        .add_embed(
            CreateEmbed::new()
                .title(&world.name)
                .footer(CreateEmbedFooter::new("Preclaim end:"))
                .timestamp(Timestamp::from_unix_timestamp(world.preclaim_end).unwrap_or_default())
                .colour(Colour::DARK_PURPLE)
                .field("Current preclaim", current_preclaim.unwrap_or(String::from("*None*")), false)
                .fields(slots.iter().map(
                    |Slot {
                         id: _,
                         name,
                         games,
                         notes,
                         points,
                         current_preclaim,
                     }| {
                        (
                            format!("`{name}`{}", if *current_preclaim { " [Current Preclaim]" } else { "" }),
                            if notes.is_empty() {
                                format!("{games}\nPoints: {points}")
                            } else {
                                format!("{games}\n*{notes}*\nPoints: {points}")
                            },
                            false,
                        )
                    },
                )),
        )
        .ephemeral(true)
        .components(components)
}
