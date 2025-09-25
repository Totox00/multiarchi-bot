use serenity::all::{AutocompleteChoice, CommandInteraction, Context, CreateAutocompleteResponse, CreateInteractionResponse};
use sqlx::query;

use crate::Bot;

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
            let filter = format!("%{partial}%");
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
}
