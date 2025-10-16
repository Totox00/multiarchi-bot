use serenity::all::{AutocompleteChoice, CommandInteraction, Context, CreateAutocompleteResponse, CreateInteractionResponse};
use sqlx::query;

use crate::{Bot, Player};

pub trait Autocomplete {
    async fn no_autocomplete(&self, _ctx: &Context);
    async fn autocomplete(&self, _ctx: &Context, options: impl Iterator<Item = impl Into<String>>);
}

impl Autocomplete for CommandInteraction {
    async fn no_autocomplete(&self, ctx: &Context) {
        let _ = self.create_response(&ctx.http, CreateInteractionResponse::Autocomplete(CreateAutocompleteResponse::new())).await;
    }

    async fn autocomplete(&self, ctx: &Context, options: impl Iterator<Item = impl Into<String>>) {
        let _ = self
            .create_response(
                &ctx.http,
                CreateInteractionResponse::Autocomplete(
                    CreateAutocompleteResponse::new().set_choices(
                        options
                            .map(|option| {
                                let as_string = option.into();
                                AutocompleteChoice::new(as_string.clone(), as_string)
                            })
                            .collect(),
                    ),
                ),
            )
            .await;
    }
}

impl Bot {
    pub async fn autocomplete_preclaim_worlds(&self, ctx: Context, interaction: &CommandInteraction, partial: &str) {
        let filter = format!("%{partial}%");
        let Ok(response) = query!("SELECT name FROM worlds WHERE name LIKE ? ORDER BY name ASC LIMIT 25", filter).fetch_all(&self.db).await else {
            interaction.no_autocomplete(&ctx).await;
            return;
        };

        interaction.autocomplete(&ctx, response.into_iter().map(|record| record.name)).await;
    }

    pub async fn autocomplete_realities(&self, ctx: Context, interaction: &CommandInteraction, partial: &str) {
        let filter = format!("%{partial}%");
        let Ok(response) = query!("SELECT name FROM realities WHERE name LIKE ? ORDER BY name ASC LIMIT 25", filter).fetch_all(&self.db).await else {
            interaction.no_autocomplete(&ctx).await;
            return;
        };

        interaction.autocomplete(&ctx, response.into_iter().map(|record| record.name)).await;
    }

    pub async fn autocomplete_worlds(&self, ctx: Context, interaction: &CommandInteraction, partial: &str) {
        let filter = format!("%{partial}%");
        let Ok(response) = query!("SELECT name FROM tracked_worlds WHERE name LIKE ? ORDER BY name ASC LIMIT 25", filter).fetch_all(&self.db).await else {
            interaction.no_autocomplete(&ctx).await;
            return;
        };

        interaction.autocomplete(&ctx, response.into_iter().map(|record| record.name)).await;
    }

    pub async fn autocomplete_slots(&self, ctx: Context, interaction: &CommandInteraction, partial: &str, world: Option<&str>) {
        let filter = format!("%{partial}%");

        if let Some(world) = world {
            let Ok(response) = query!(
                "SELECT name FROM tracked_slots WHERE world IN (SELECT id FROM tracked_worlds WHERE name = ?) AND name LIKE ? ORDER BY name ASC LIMIT 25",
                world,
                filter
            )
            .fetch_all(&self.db)
            .await
            else {
                interaction.no_autocomplete(&ctx).await;
                return;
            };

            interaction.autocomplete(&ctx, response.into_iter().map(|record| record.name)).await;
        } else {
            let Ok(response) = query!("SELECT name FROM tracked_slots WHERE name LIKE ? ORDER BY name ASC LIMIT 25", filter).fetch_all(&self.db).await else {
                interaction.no_autocomplete(&ctx).await;
                return;
            };
            interaction.autocomplete(&ctx, response.into_iter().map(|record| record.name)).await;
        }
    }

    pub async fn autocomplete_slots_claimed(&self, ctx: Context, interaction: &CommandInteraction, partial: &str, world: Option<&str>, player: &Player) {
        let filter = format!("%{partial}%");
        let mut recommendations = vec![];

        if let Some(world) = world {
            if let Ok(response) = query!(
                "SELECT name FROM tracked_slots LEFT JOIN claims ON claims.slot = tracked_slots.id WHERE claims.player = ? AND world IN (SELECT id FROM tracked_worlds WHERE name = ?) AND name LIKE ? ORDER BY name ASC LIMIT 25",
                player.id,
                world,
                filter
            )
            .fetch_all(&self.db)
            .await {
                recommendations.extend(response.into_iter().map(|record| record.name));
            }

            let remaining = 25 - recommendations.len() as i64;
            if let Ok(response) = query!(
                "SELECT name FROM tracked_slots LEFT JOIN claims ON claims.slot = tracked_slots.id WHERE (claims.player != ? OR claims.player IS NULL) AND world IN (SELECT id FROM tracked_worlds WHERE name = ?) AND name LIKE ? ORDER BY name ASC LIMIT ?",
                player.id,
                world,
                filter,
                remaining
            )
            .fetch_all(&self.db)
            .await
            {
                recommendations.extend(response.into_iter().map(|record| record.name));
            }
        } else {
            if let Ok(response) = query!(
                "SELECT name FROM tracked_slots LEFT JOIN claims ON claims.slot = tracked_slots.id WHERE claims.player = ? AND name LIKE ? ORDER BY name ASC LIMIT 25",
                player.id,
                filter
            )
            .fetch_all(&self.db)
            .await
            {
                recommendations.extend(response.into_iter().map(|record| record.name));
            }

            let remaining = 25 - recommendations.len() as i64;
            if let Ok(response) = query!(
                "SELECT name FROM tracked_slots LEFT JOIN claims ON claims.slot = tracked_slots.id WHERE (claims.player != ? OR claims.player IS NULL) AND name LIKE ? ORDER BY name ASC LIMIT ?",
                player.id,
                filter,
                remaining
            )
            .fetch_all(&self.db)
            .await
            {
                recommendations.extend(response.into_iter().map(|record| record.name));
            }
        }

        if recommendations.is_empty() {
            interaction.no_autocomplete(&ctx).await;
        } else {
            interaction.autocomplete(&ctx, recommendations.into_iter()).await;
        }
    }
}
