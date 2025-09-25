use std::collections::HashMap;

use serenity::all::{
    AutocompleteOption, ButtonStyle, Colour, CommandInteraction, CommandOptionType, CommandType, ComponentInteraction, Context, CreateActionRow, CreateButton, CreateCommand, CreateCommandOption,
    CreateEmbed, CreateInteractionResponse, CreateInteractionResponseMessage, ResolvedOption, ResolvedValue,
};
use sqlx::query;

use crate::{
    autocomplete::Autocomplete,
    commands::Command,
    util::{get_page, SimpleReply},
    Bot,
};

struct World {
    id: i64,
    name: String,
    slots: Vec<i64>,
}

struct Slot {
    name: String,
    games: String,
    free: bool,
}

pub struct UnclaimedCommand {}

impl Command for UnclaimedCommand {
    const NAME: &'static str = "unclaimed";

    fn register() -> CreateCommand {
        CreateCommand::new(Self::NAME)
            .description("View unclaimed slots")
            .kind(CommandType::ChatInput)
            .add_option(CreateCommandOption::new(CommandOptionType::String, "world", "Name of the world").required(false).set_autocomplete(true))
    }

    async fn execute(bot: &Bot, ctx: Context, command: CommandInteraction) {
        let mut world_name = "";

        for ResolvedOption { name: option_name, value, .. } in command.data.options() {
            if let ("world", ResolvedValue::String(value)) = (option_name, value) {
                world_name = value
            }
        }

        let Some(worlds) = get_worlds(bot).await else {
            command.simple_reply(&ctx, "Failed to get current worlds").await;
            return;
        };

        if worlds.is_empty() {
            command.simple_reply(&ctx, "There are no worlds with unclaimed slots").await;
            return;
        }

        let (page_count, start_page) = if world_name.is_empty() {
            (worlds.iter().map(|world| if world.slots.len() <= 25 { 1 } else { world.slots.len().div_ceil(20) }).sum(), 0)
        } else {
            let mut page_count = 0;
            let mut start_page = 0;

            for world in &worlds {
                if world.name == world_name {
                    start_page = page_count;
                }

                if world.slots.len() <= 25 {
                    page_count += 1;
                } else {
                    page_count += world.slots.len().div_ceil(20);
                }
            }

            (page_count, start_page)
        };

        let (world, slots) = if let Some((world, slots)) = get_page(&worlds, |world| &world.slots, start_page) {
            (world, get_slot_details(bot, slots).await)
        } else {
            command.simple_reply(&ctx, "Invalid page").await;
            return;
        };

        let _ = command
            .create_response(&ctx.http, CreateInteractionResponse::Message(build_embed(world, slots, start_page, page_count)))
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

impl UnclaimedCommand {
    pub async fn handle_interraction(bot: &Bot, ctx: Context, interaction: &ComponentInteraction, id: &str) {
        if let Some((_, rest)) = id.split_once("page-") {
            if let Ok(new_page) = rest.parse::<usize>() {
                let Some(worlds) = get_worlds(bot).await else {
                    interaction.simple_reply(&ctx, "Failed to get current worlds").await;
                    return;
                };

                let page_count = worlds.iter().map(|world| if world.slots.len() <= 25 { 1 } else { world.slots.len().div_ceil(20) }).sum();

                if new_page >= page_count {
                    interaction.simple_reply(&ctx, "Invalid page").await;
                    return;
                }

                let (world, slots) = if let Some((world, slots)) = get_page(&worlds, |world| &world.slots, new_page) {
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
    if let Ok(response) = query!("SELECT tracked_worlds.id as world_id, tracked_worlds.name AS world_name, tracked_slots.id AS slot_id FROM tracked_worlds INNER JOIN tracked_slots ON tracked_worlds.id = tracked_slots.world WHERE tracked_slots.id NOT IN (SELECT slot FROM claims) ORDER BY tracked_slots.name").fetch_all(&bot.db).await {
        let mut worlds: HashMap<i64, (String, Vec<i64>)> = HashMap::new();

        for record in response {
            worlds
                .entry(record.world_id)
                .and_modify(|(_, vec)| vec.push(record.slot_id))
                .or_insert((record.world_name, vec![record.slot_id]));
        }

        let mut worlds_vec: Vec<_> = worlds.into_iter().map(|(id, (name, slots))| World { id, name, slots }).collect();
        worlds_vec.sort_by_key(|world| world.id);
        Some(worlds_vec)
    } else {
        None
    }
}

async fn get_slot_details(bot: &Bot, slots: &[i64]) -> Vec<Slot> {
    let mut out = vec![];

    for slot in slots {
        if let Ok(response) = query!("SELECT name, games, free FROM tracked_slots WHERE id = ? LIMIT 1", slot).fetch_one(&bot.db).await {
            out.push(Slot {
                name: response.name,
                games: response.games,
                free: response.free > 0,
            });
        }
    }

    out
}

fn build_embed(world: &World, slots: Vec<Slot>, page: usize, page_count: usize) -> CreateInteractionResponseMessage {
    let mut components = vec![];

    if page_count > 1 {
        components.push(CreateActionRow::Buttons(vec![
            CreateButton::new(format!("unclaimed-page-{}", page - 1))
                .disabled(page == 0)
                .style(ButtonStyle::Secondary)
                .label("← Prev"),
            CreateButton::new("unclaimed-page-count")
                .disabled(true)
                .style(ButtonStyle::Secondary)
                .label(format!("Page {} of {page_count}", page + 1)),
            CreateButton::new(format!("unclaimed-page-{}", page + 1))
                .disabled(page + 1 == page_count)
                .style(ButtonStyle::Secondary)
                .label("Next →"),
        ]));
    }

    CreateInteractionResponseMessage::new()
        .add_embed(
            CreateEmbed::new().title(&world.name).colour(Colour::DARK_PURPLE).fields(
                slots
                    .iter()
                    .map(|Slot { name, games, free }| (name, format!("{games}{}", if *free { "\n*Free claim*" } else { "" }), false)),
            ),
        )
        .ephemeral(true)
        .components(components)
}
