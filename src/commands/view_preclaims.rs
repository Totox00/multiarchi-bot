use std::collections::HashMap;

use serenity::all::{
    Colour, CommandInteraction, CommandType, ComponentInteraction, ComponentInteractionDataKind, Context, CreateActionRow, CreateCommand, CreateEmbed, CreateEmbedFooter, CreateInteractionResponse,
    CreateSelectMenu, CreateSelectMenuKind, CreateSelectMenuOption, Timestamp,
};
use sqlx::query;

use crate::{
    commands::Command,
    paginate::{PageContainer, PageDetails, PageItem, Paginate},
    util::SimpleReply,
    Bot, Player,
};

pub struct World {
    name: String,
    preclaim_end: i64,
    slots: Vec<SlotId>,
}

pub struct SlotId(i64);

pub struct Slot {
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

        let Some(response) = Self::first_page(bot, &player).await else {
            command.simple_reply(&ctx, "There are no worlds currently accepting preclaims").await;
            return;
        };

        let _ = command.create_response(&ctx.http, CreateInteractionResponse::Message(response.into())).await;
    }
}

impl Paginate<World, SlotId, Slot, &Player> for ViewPreclaimsCommand {
    const PAGE_SIZE: usize = 24;

    async fn get_containers(bot: &Bot, _: &Player) -> Vec<World> {
        if let Ok(response) = query!("SELECT worlds.name AS world_name, preclaim_end, slots.id AS slot_id FROM worlds INNER JOIN slots ON worlds.id = slots.world WHERE preclaim_end > strftime('%s', 'now') ORDER BY slots.name").fetch_all(&bot.db).await {
            let mut worlds: HashMap<String, (i64, Vec<SlotId>)> = HashMap::new();

            for record in response {
                worlds
                    .entry(record.world_name)
                    .and_modify(|(_, vec)| vec.push(SlotId(record.slot_id)))
                    .or_insert((record.preclaim_end, vec![SlotId(record.slot_id)]));
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
            worlds_vec
        } else {
            vec![]
        }
    }

    fn additional_components(components: &mut Vec<CreateActionRow>, _container: &World, details: &[Slot]) {
        components.push(CreateActionRow::SelectMenu(
            CreateSelectMenu::new(
                "view-preclaims-select",
                CreateSelectMenuKind::String {
                    options: details.iter().map(|slot| CreateSelectMenuOption::new(&slot.name, slot.id.to_string())).collect(),
                },
            )
            .placeholder("Make preclaim")
            .min_values(1)
            .max_values(1),
        ));
    }

    async fn additional_fields(bot: &Bot, fields: &mut Vec<(String, String, bool)>, player: &Player) {
        let current_preclaim = query!("SELECT name FROM slots INNER JOIN preclaims ON preclaims.slot = slots.id WHERE status = 0 AND player = ?", player.id)
            .fetch_one(&bot.db)
            .await
            .map(|record| record.name)
            .ok();

        fields.push((String::from("Current preclaim"), current_preclaim.unwrap_or(String::from("*None*")), false));
    }
}

impl PageContainer<SlotId, Slot, &Player> for World {
    fn items(&self) -> &[SlotId] {
        &self.slots
    }

    fn page_setup(&self) -> CreateEmbed {
        CreateEmbed::new()
            .title(&self.name)
            .footer(CreateEmbedFooter::new("Preclaim end:"))
            .timestamp(Timestamp::from_unix_timestamp(self.preclaim_end).unwrap_or_default())
            .colour(Colour::DARK_PURPLE)
    }
}

impl PageItem<Slot, &Player> for SlotId {
    async fn details(&self, bot: &Bot, player: &Player) -> Option<Slot> {
        let response = query!("SELECT name, games, notes, points FROM slots WHERE id = ? LIMIT 1", self.0).fetch_one(&bot.db).await.ok()?;
        let current_preclaim = query!("SELECT status FROM preclaims WHERE slot = ? AND player = ? LIMIT 1", self.0, player.id)
            .fetch_one(&bot.db)
            .await
            .is_ok();

        Some(Slot {
            id: self.0,
            name: response.name,
            games: response.games,
            notes: response.notes,
            points: response.points,
            current_preclaim,
        })
    }
}

impl PageDetails for Slot {
    fn field(&self) -> (String, String, bool) {
        (
            format!("`{}`{}", self.name, if self.current_preclaim { " [Current Preclaim]" } else { "" }),
            if self.notes.is_empty() {
                format!("{}\nPoints: {}", self.games, self.points)
            } else {
                format!("{}\n*{}*\nPoints: {}", self.games, self.notes, self.points)
            },
            false,
        )
    }
}

impl ViewPreclaimsCommand {
    pub async fn handle_interraction(bot: &Bot, ctx: Context, interaction: &ComponentInteraction, id: &str) {
        let user_id = i64::from(interaction.user.id);
        let Some(player) = bot.get_player(user_id, &interaction.user.name).await else {
            interaction.simple_reply(&ctx, "Failed to get player").await;
            return;
        };

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
                    interaction.simple_reply(&ctx, "Preclaims must be of exactly one slot").await;
                    return;
                }
            } else {
                interaction.simple_reply(&ctx, "Malformed selection data").await;
                return;
            };

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

            interaction.simple_reply(&ctx, "Successfully preclaimed slot").await;
        } else if Self::try_handle_interaction(bot, &ctx, interaction, id, &player).await {
        } else {
            interaction.simple_reply(&ctx, "Unrecognized interraction").await;
        }
    }
}
