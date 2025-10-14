use std::collections::HashMap;

use serenity::all::{
    AutocompleteOption, Colour, CommandInteraction, CommandOptionType, CommandType, ComponentInteraction, Context, CreateCommand, CreateCommandOption, CreateEmbed, CreateInteractionResponse,
    ResolvedOption, ResolvedValue,
};
use sqlx::query;

use crate::{
    autocomplete::Autocomplete,
    commands::Command,
    paginate::{PageContainer, PageDetails, PageItem, Paginate},
    util::SimpleReply,
    Bot,
};

struct World {
    id: i64,
    name: String,
    slots: Vec<SlotId>,
}

pub struct SlotId(i64);

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

        let worlds = Self::get_containers(bot).await;

        if worlds.is_empty() {
            command.simple_reply(&ctx, "There are no worlds with unclaimed slots").await;
            return;
        }

        let start_page = if world_name.is_empty() {
            0
        } else {
            let mut start_page = 0;

            for world in &worlds {
                if world.name == world_name {
                    break;
                }

                if world.slots.len() <= Self::PAGE_SIZE {
                    start_page += 1;
                } else {
                    start_page += world.slots.len().div_ceil(Self::PAGE_SIZE);
                }
            }

            start_page
        };

        let Some(response) = Self::page(bot, (), start_page).await else {
            command.simple_reply(&ctx, "There are no worlds with unclaimed slots").await;
            return;
        };

        let _ = command.create_response(&ctx.http, CreateInteractionResponse::Message(response)).await;
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

impl Paginate<World, SlotId, Slot> for UnclaimedCommand {
    async fn get_containers(bot: &Bot) -> Vec<World> {
        if let Ok(response) = query!("SELECT tracked_worlds.id as world_id, tracked_worlds.name AS world_name, tracked_slots.id AS slot_id FROM tracked_worlds INNER JOIN tracked_slots ON tracked_worlds.id = tracked_slots.world WHERE tracked_slots.id NOT IN (SELECT slot FROM claims) ORDER BY tracked_slots.name").fetch_all(&bot.db).await {
            let mut worlds: HashMap<i64, (String, Vec<SlotId>)> = HashMap::new();

            for record in response {
                worlds
                    .entry(record.world_id)
                    .and_modify(|(_, vec)| vec.push(SlotId(record.slot_id)))
                    .or_insert((record.world_name, vec![SlotId(record.slot_id)]));
            }

            let mut worlds_vec: Vec<_> = worlds.into_iter().map(|(id, (name, slots))| World { id, name, slots }).collect();
            worlds_vec.sort_by_key(|world| world.id);
            worlds_vec
        } else {
            vec![]
        }
    }
}

impl PageContainer<SlotId, Slot> for World {
    fn items(&self) -> &[SlotId] {
        &self.slots
    }

    fn page_setup(&self) -> CreateEmbed {
        CreateEmbed::new().title(&self.name).colour(Colour::DARK_PURPLE)
    }
}

impl PageItem<Slot> for SlotId {
    async fn details(&self, bot: &Bot, _extra: ()) -> Option<Slot> {
        let response = query!("SELECT name, games, free FROM tracked_slots WHERE id = ? LIMIT 1", self.0).fetch_one(&bot.db).await.ok()?;

        Some(Slot {
            name: response.name,
            games: response.games,
            free: response.free > 0,
        })
    }
}

impl PageDetails for Slot {
    fn field(&self) -> (String, String, bool) {
        (self.name.to_owned(), format!("{}{}", self.games, if self.free { "\n*Free claim*" } else { "" }), false)
    }
}

impl UnclaimedCommand {
    pub async fn handle_interraction(bot: &Bot, ctx: Context, interaction: &ComponentInteraction, id: &str) {
        if Self::try_handle_interaction(bot, &ctx, interaction, id, ()).await {
        } else {
            interaction.simple_reply(&ctx, "Unrecognized interraction").await;
        }
    }
}
