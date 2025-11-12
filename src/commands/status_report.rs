use std::fmt::Display;

use serenity::all::{
    AutocompleteOption, Colour, CommandInteraction, CommandOptionType, CommandType, ComponentInteraction, ComponentInteractionDataKind, Context, CreateActionRow, CreateCommand, CreateCommandOption,
    CreateEmbed, CreateEmbedFooter, CreateInteractionResponse, CreateSelectMenu, CreateSelectMenuKind, CreateSelectMenuOption, EditInteractionResponse, ResolvedOption, ResolvedValue,
};
use sqlx::query;

use crate::{
    autocomplete::Autocomplete,
    commands::Command,
    paginate::{PageContainer, PageDetails, PageItem, Paginate},
    scrape::Status,
    util::SimpleReply,
    Bot,
};

#[derive(Clone, Copy)]
enum Sort {
    Sent,
    Checks,
}

struct World {
    name: String,
    checks: i64,
    checks_total: i64,
    slots: Vec<SlotId>,
}

struct SlotId(i64);

struct Slot {
    id: i64,
    name: String,
    player: String,
    status: i64,
    checks: i64,
    checks_total: i64,
    last_activity: Option<i64>,
    updates: Vec<Update>,
}

struct Update {
    timestamp: i64,
    player: i64,
    description: String,
}

#[derive(Clone, Copy)]
struct Extra {
    world_id: i64,
    sort: Sort,
}

pub struct StatusReportCommand {}

impl Command for StatusReportCommand {
    const NAME: &'static str = "status-report";

    fn register() -> CreateCommand {
        CreateCommand::new(Self::NAME)
            .description("Gets a status report")
            .kind(CommandType::ChatInput)
            .add_option(CreateCommandOption::new(CommandOptionType::String, "world", "Name of the world").required(true).set_autocomplete(true))
            .add_option(
                CreateCommandOption::new(CommandOptionType::String, "sort", "How to sort the slot")
                    .required(false)
                    .add_string_choice("Last check sent", "sent")
                    .add_string_choice("Checks done", "checks"),
            )
    }

    async fn execute(bot: &Bot, ctx: Context, command: CommandInteraction) {
        let mut world = "";
        let mut sort = "sent";

        for ResolvedOption { name: option_name, value, .. } in command.data.options() {
            match (option_name, value) {
                ("world", ResolvedValue::String(value)) => world = value,
                ("sort", ResolvedValue::String(value)) => sort = value,
                _ => (),
            }
        }

        if world.is_empty() {
            command.simple_reply(&ctx, "A world is required").await;
            return;
        }

        let Some(sort) = Sort::try_from(sort) else {
            command.simple_reply(&ctx, "Invalid sort option").await;
            return;
        };

        let world_id = if let Ok(response) = query!("SELECT id FROM tracked_worlds WHERE name = ? LIMIT 1", world).fetch_one(&bot.db).await {
            if let Some(id) = response.id {
                id
            } else {
                command.simple_reply(&ctx, "Failed to get world").await;
                return;
            }
        } else {
            command.simple_reply(&ctx, "Failed to get world").await;
            return;
        };

        let _ = command.defer_ephemeral(&ctx).await;

        if bot.privileged.contains(&command.user.id) {
            bot.update_scrape(world).await;
            bot.push_needed().await;
        }

        let Some(response) = Self::first_page(bot, Extra { world_id, sort }).await else {
            let _ = command.edit_response(&ctx, EditInteractionResponse::new().content("Failed to get status report")).await;
            return;
        };

        let _ = command.edit_response(&ctx, response.into()).await;
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

impl Sort {
    fn try_from(value: &str) -> Option<Self> {
        match value {
            "sent" => Some(Sort::Sent),
            "checks" => Some(Sort::Checks),
            _ => None,
        }
    }

    fn as_i(self) -> usize {
        match self {
            Sort::Sent => 0,
            Sort::Checks => 1,
        }
    }

    fn try_from_i(value: usize) -> Option<Sort> {
        match value {
            0 => Some(Sort::Sent),
            1 => Some(Sort::Checks),
            _ => None,
        }
    }
}

impl Paginate<World, SlotId, Slot, Extra> for StatusReportCommand {
    const PAGE_SIZE: usize = 10;

    async fn get_containers(bot: &Bot, Extra { world_id, sort }: Extra) -> Vec<World> {
        let Ok(world_name) = query!("SELECT name FROM tracked_worlds WHERE id = ? LIMIT 1", world_id).fetch_one(&bot.db).await else {
            return vec![];
        };

        let Ok(slot_response) = (match sort {
            Sort::Sent => query!(
                "SELECT id, checks, checks_total FROM tracked_slots WHERE status < 3 AND world = ? ORDER BY last_activity DESC NULLS FIRST",
                world_id
            )
            .fetch_all(&bot.db)
            .await
            .map(|response| response.into_iter().map(|record| (record.id, record.checks, record.checks_total)).collect::<Vec<_>>()),
            Sort::Checks => query!(
                "SELECT id, checks, checks_total FROM tracked_slots WHERE status < 3 AND world = ? ORDER BY (CAST(checks AS REAL) / CAST(checks_total AS REAL)) ASC",
                world_id
            )
            .fetch_all(&bot.db)
            .await
            .map(|response| response.into_iter().map(|record| (record.id, record.checks, record.checks_total)).collect::<Vec<_>>()),
        }) else {
            return vec![];
        };

        let mut all_checks = 0;
        let mut all_checks_total = 0;
        let mut slots = vec![];

        for (slot, checks, checks_total) in slot_response {
            all_checks += checks;
            all_checks_total += checks_total;
            slots.push(SlotId(slot));
        }

        vec![World {
            name: world_name.name,
            checks: all_checks,
            checks_total: all_checks_total,
            slots,
        }]
    }

    fn additional_components(components: &mut Vec<CreateActionRow>, _container: &World, details: &[Slot]) {
        components.push(CreateActionRow::SelectMenu(
            CreateSelectMenu::new(
                "status-report-select",
                CreateSelectMenuKind::String {
                    options: details.iter().map(|slot| CreateSelectMenuOption::new(&slot.name, slot.id.to_string())).collect(),
                },
            )
            .placeholder("View slot report")
            .min_values(1)
            .max_values(1),
        ));
    }

    fn id_prefix(_container: &World, extra: Extra) -> impl Display {
        format!("{}-{}-{}", Self::NAME, extra.world_id, extra.sort.as_i())
    }
}

impl PageContainer<SlotId, Slot, Extra> for World {
    fn items(&self) -> &[SlotId] {
        &self.slots
    }

    fn page_setup(&self) -> CreateEmbed {
        CreateEmbed::new()
            .title(&self.name)
            .colour(Colour::DARK_PURPLE)
            .footer(CreateEmbedFooter::new(format!("Total checks: {}/{}", self.checks, self.checks_total)))
    }
}

impl PageItem<Slot, Extra> for SlotId {
    async fn details(&self, bot: &Bot, _extra: Extra) -> Option<Slot> {
        let response = query!("SELECT name, status, checks, checks_total, last_activity FROM tracked_slots WHERE id = ? LIMIT 1", self.0)
            .fetch_one(&bot.db)
            .await
            .ok()?;
        let player = if let Ok(response) = query!("SELECT snowflake FROM claims INNER JOIN players ON claims.player = players.id WHERE slot = ?", self.0)
            .fetch_one(&bot.db)
            .await
        {
            format!("**Claimed by**: <@{}>", response.snowflake)
        } else {
            String::from("*Unclaimed*")
        };
        let mut updates = vec![];
        if let Ok(status_response) = query!("SELECT timestamp, players.snowflake as player_id, description FROM updates INNER JOIN players ON updates.player = players.id WHERE slot = ? GROUP BY updates.player HAVING timestamp = MAX(timestamp) ORDER BY timestamp DESC LIMIT 8", self.0).fetch_all(&bot.db).await {
            for record in status_response {
                if let (Some(timestamp), Some(player), Some(mut description)) = (record.timestamp, record.player_id, record.description) {
                    if description.len() > 120 {
                        description.truncate(100);
                        description.push_str("...");
                    }
                    updates.push(Update { timestamp, player, description });
                }
            }
        }

        Some(Slot {
            id: self.0,
            name: response.name,
            player,
            status: response.status,
            checks: response.checks,
            checks_total: response.checks_total,
            last_activity: response.last_activity,
            updates,
        })
    }
}

impl PageDetails for Slot {
    fn field(&self) -> (String, String, bool) {
        (
            format!("{} ({}/{})", self.name, self.checks, self.checks_total),
            format!(
                "{}\n**Status**: {}\n**Last activity**: {}\n{}",
                self.player,
                Status::from_i64(self.status).map(|status| status.as_str()).unwrap_or("Unknown"),
                if let Some(minutes) = self.last_activity {
                    format!("{} hours, {} minutes ago", minutes / 60, minutes % 60)
                } else {
                    String::from("Never")
                },
                self.updates
                    .iter()
                    .map(|Update { timestamp, player, description }| format!("[<t:{timestamp}:f>] [<@{player}>] {description}",))
                    .collect::<Vec<_>>()
                    .join("\n")
            ),
            false,
        )
    }
}

impl StatusReportCommand {
    pub async fn handle_interraction(bot: &Bot, ctx: Context, interaction: &ComponentInteraction, id: &str) {
        if id == "select" {
            let slot_id = if let ComponentInteractionDataKind::StringSelect { values } = &interaction.data.kind {
                if let Some(slot_id) = values.first() {
                    if let Ok(slot_id) = slot_id.parse::<i64>() {
                        slot_id
                    } else {
                        interaction.simple_reply(&ctx, "Malformed slot id").await;
                        return;
                    }
                } else {
                    interaction.simple_reply(&ctx, "Detailed report must be of exactly one slot").await;
                    return;
                }
            } else {
                interaction.simple_reply(&ctx, "Malformed selection data").await;
                return;
            };

            let _ = interaction.defer_ephemeral(&ctx.http).await;

            if let Ok(slot_response) = query!("SELECT name, games, status, checks, checks_total, last_activity FROM tracked_slots WHERE id = ? LIMIT 1", slot_id)
                .fetch_one(&bot.db)
                .await
            {
                if let Ok(status_response) = query!(
                    "SELECT timestamp, players.snowflake as player_id, description FROM updates INNER JOIN players ON updates.player = players.id WHERE slot = ? ORDER BY timestamp DESC LIMIT 20",
                    slot_id,
                )
                .fetch_all(&bot.db)
                .await
                {
                    let player_str = if let Ok(response) = query!("SELECT snowflake FROM claims INNER JOIN players ON claims.player = players.id WHERE slot = ?", slot_id)
                        .fetch_one(&bot.db)
                        .await
                    {
                        format!("**Claimed by**: <@{}>", response.snowflake)
                    } else {
                        String::from("*Unclaimed*")
                    };

                    let _ = interaction
                        .edit_response(
                            &ctx.http,
                            EditInteractionResponse::new().add_embed(
                                CreateEmbed::new()
                                    .title(slot_response.name)
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
                let _ = interaction.edit_response(&ctx.http, EditInteractionResponse::new().content("Failed to get slot info")).await;
            }
        } else if let [world_id, sort, "page", page] = id.split('-').collect::<Vec<_>>().as_slice() {
            if let Ok(new_page) = page.parse::<usize>() {
                let Ok(world_id) = world_id.parse() else {
                    interaction.simple_reply(&ctx, "Malformed world id").await;
                    return;
                };

                let Some(sort) = sort.parse().ok().and_then(Sort::try_from_i) else {
                    interaction.simple_reply(&ctx, "Malformed sort type").await;
                    return;
                };

                let extra = Extra { world_id, sort };

                let Some(response) = Self::page(bot, extra, new_page).await else {
                    interaction.simple_reply(&ctx, "There was an error handling your interraction").await;
                    return;
                };

                let _ = interaction.create_response(ctx, CreateInteractionResponse::UpdateMessage(response.into())).await;
            } else {
                interaction.simple_reply(&ctx, "Malformed page number").await;
            }
        } else {
            interaction.simple_reply(&ctx, "Unrecognized interraction").await;
        }
    }
}
