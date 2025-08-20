use std::collections::HashMap;

use serenity::{
    all::{
        ButtonStyle, Colour, CommandInteraction, CommandType, ComponentInteraction, ComponentInteractionDataKind, Context, CreateActionRow, CreateButton, CreateCommand, CreateEmbed,
        CreateEmbedFooter, CreateInteractionResponse, CreateInteractionResponseMessage, CreateSelectMenu, CreateSelectMenuKind, CreateSelectMenuOption, Timestamp,
    },
    futures::future::join_all,
};
use sqlx::query;

use crate::{util::SimpleReply, Bot};

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
}

pub struct ViewPreclaimsCommand {}

impl ViewPreclaimsCommand {
    pub fn register() -> CreateCommand {
        CreateCommand::new("view-preclaims").description("View worlds with possible preclaims").kind(CommandType::ChatInput)
    }

    pub async fn execute(bot: &Bot, ctx: Context, command: CommandInteraction) {
        let worlds = if let Some(worlds) = get_worlds(bot).await {
            worlds
        } else {
            command.simple_reply(&ctx, "Failed to get current worlds").await;
            return;
        };

        let first_world = if let Some(world) = worlds.first() {
            world
        } else {
            command.simple_reply(&ctx, "There are no worlds currently accepting preclaims").await;
            return;
        };

        let page_count = worlds.iter().map(|world| if world.slots.len() <= 25 { 1 } else { world.slots.len().div_ceil(20) }).sum();

        let _ = command
            .create_response(
                &ctx.http,
                CreateInteractionResponse::Message(build_embed(
                    first_world,
                    get_slot_details(bot, if first_world.slots.len() <= 25 { &first_world.slots[..] } else { &first_world.slots[0..20] }).await,
                    0,
                    page_count,
                )),
            )
            .await;
    }

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

            if query!("DELETE FROM preclaims WHERE user_snowflake = ? AND old = 0", user_id).execute(&bot.db).await.is_err() {
                interaction.simple_reply(&ctx, "Failed to remove old preclaims").await;
                return;
            }

            if query!("INSERT INTO preclaims (slot, user_snowflake) VALUES (?, ?)", slot_id, user_id).execute(&bot.db).await.is_err() {
                interaction.simple_reply(&ctx, "Failed to create preclaim").await;
                return;
            }

            interaction.simple_reply(&ctx, "Successfully preclaimed slot").await;
        } else if let Some((_, rest)) = id.split_once("page-") {
            if let Ok(new_page) = rest.parse::<usize>() {
                let worlds = if let Some(worlds) = get_worlds(bot).await {
                    worlds
                } else {
                    interaction.simple_reply(&ctx, "Failed to get current worlds").await;
                    return;
                };

                let page_count = worlds.iter().map(|world| if world.slots.len() <= 25 { 1 } else { world.slots.len().div_ceil(20) }).sum();

                if new_page >= page_count {
                    interaction.simple_reply(&ctx, "Invalid page").await;
                    return;
                }

                let (world, slots) = if let Some((world, slots)) = get_correct_page(&worlds, new_page) {
                    (world, get_slot_details(bot, slots).await)
                } else {
                    interaction.simple_reply(&ctx, "Invalid page").await;
                    return;
                };

                let _ = interaction
                    .create_response(&ctx.http, CreateInteractionResponse::UpdateMessage(build_embed(world, slots, new_page, page_count)))
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
    if let Ok(response) =
        query!("SELECT worlds.name AS world_name, preclaim_end, slots.id AS slot_id FROM worlds INNER JOIN slots ON worlds.id = slots.world WHERE preclaim_end > strftime('%s', 'now')")
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

async fn get_slot_details(bot: &Bot, slots: &[i64]) -> Vec<Slot> {
    join_all(
        slots
            .iter()
            .map(async |id| (id, query!("SELECT name, games, notes FROM slots WHERE id = ? LIMIT 1", id).fetch_one(&bot.db).await)),
    )
    .await
    .into_iter()
    .filter_map(|(id, response)| {
        if let Ok(record) = response {
            Some(Slot {
                id: *id,
                name: record.name,
                games: record.games,
                notes: record.notes,
            })
        } else {
            None
        }
    })
    .collect()
}

fn get_correct_page(worlds: &[World], mut page: usize) -> Option<(&World, &[i64])> {
    let mut worlds_iter = worlds.iter();

    let mut current_world = worlds_iter.next()?;
    let mut current_start = 0;

    while page != 0 {
        if current_world.slots.len() <= 25 || current_world.slots.len() - current_start <= 20 {
            current_world = worlds_iter.next()?;
        } else {
            current_start += 20;
        }

        page -= 1;
    }

    if current_world.slots.len() <= 25 {
        Some((current_world, current_world.slots.as_slice()))
    } else {
        let end = current_world.slots.len().min(current_start + 20);
        Some((current_world, &current_world.slots[current_start..end]))
    }
}

fn build_embed(world: &World, slots: Vec<Slot>, page: usize, page_count: usize) -> CreateInteractionResponseMessage {
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
                .fields(
                    slots
                        .iter()
                        .map(|Slot { id: _, name, games, notes }| (format!("`{name}`"), if notes.is_empty() { games.to_owned() } else { format!("{games}\n*{notes}*") }, false)),
                ),
        )
        .ephemeral(true)
        .components(components)
}
