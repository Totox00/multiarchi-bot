use std::fmt::Display;

use serenity::{
    all::{ButtonStyle, ComponentInteraction, Context, CreateActionRow, CreateButton, CreateEmbed, CreateInteractionResponse, CreateInteractionResponseMessage, EditInteractionResponse},
    futures::future::join_all,
};

use crate::{commands::Command, util::SimpleReply, Bot};

pub trait Paginate<Container, Item, Details, ExtraPageDetails = ()>
where
    Self: Command,
    Container: PageContainer<Item, Details, ExtraPageDetails>,
    Item: PageItem<Details, ExtraPageDetails>,
    Details: PageDetails,
    ExtraPageDetails: Copy,
{
    const PAGE_SIZE: usize = 25;

    async fn get_containers(bot: &Bot, extra: ExtraPageDetails) -> Vec<Container>;
    fn additional_components(_components: &mut Vec<CreateActionRow>, _container: &Container, _details: &[Details]) {}
    async fn additional_fields(_bot: &Bot, _fields: &mut Vec<(String, String, bool)>, _extra: ExtraPageDetails) {}
    fn id_prefix(_container: &Container, _extra: ExtraPageDetails) -> impl Display {
        Self::NAME
    }

    fn page_count(containers: &[Container]) -> usize {
        containers.iter().map(|container| container.items().len().div_ceil(Self::PAGE_SIZE)).sum()
    }

    fn get_page(containers: &[Container], mut page: usize) -> Option<(&Container, &[Item])> {
        let mut container_iter = containers.iter();

        let mut current_container = container_iter.next()?;
        let mut current_start = 0;
        let mut current_items = current_container.items();

        while page != 0 {
            if current_items.len() - current_start <= Self::PAGE_SIZE {
                current_container = container_iter.next()?;
                current_items = current_container.items();
                current_start = 0;
            } else {
                current_start += Self::PAGE_SIZE;
            }

            page -= 1;
        }

        let end = current_items.len().min(current_start + Self::PAGE_SIZE);
        Some((current_container, &current_items[current_start..end]))
    }

    async fn build_embed(bot: &Bot, container: &Container, details: &[Details], page: usize, page_count: usize, extra: ExtraPageDetails) -> Response {
        let mut components = vec![];

        if page_count > 1 {
            components.push(CreateActionRow::Buttons(vec![
                CreateButton::new(format!("{}-page-{}", Self::id_prefix(container, extra), page - 1))
                    .disabled(page == 0)
                    .style(ButtonStyle::Secondary)
                    .label("← Prev"),
                CreateButton::new(format!("{}-page-count", Self::id_prefix(container, extra)))
                    .disabled(true)
                    .style(ButtonStyle::Secondary)
                    .label(format!("Page {} of {page_count}", page + 1)),
                CreateButton::new(format!("{}-page-{}", Self::id_prefix(container, extra), page + 1))
                    .disabled(page + 1 == page_count)
                    .style(ButtonStyle::Secondary)
                    .label("Next →"),
            ]));
        }

        Self::additional_components(&mut components, container, details);

        let mut fields = vec![];
        Self::additional_fields(bot, &mut fields, extra).await;

        Response::new()
            .add_embed(container.page_setup().fields(fields).fields(details.iter().map(PageDetails::field)))
            .components(components)
    }

    async fn first_page(bot: &Bot, extra: ExtraPageDetails) -> Option<Response> {
        let containers = Self::get_containers(bot, extra).await;
        let first_container = containers.first()?;
        let page_count = Self::page_count(&containers);
        let details: Vec<_> = join_all(first_container.items().iter().take(Self::PAGE_SIZE).map(|item| item.details(bot, extra)))
            .await
            .into_iter()
            .flatten()
            .collect();

        Some(Self::build_embed(bot, first_container, &details, 0, page_count, extra).await)
    }

    async fn page(bot: &Bot, extra: ExtraPageDetails, page: usize) -> Option<Response> {
        let containers = Self::get_containers(bot, extra).await;
        let page_count = Self::page_count(&containers);

        if page >= page_count {
            return Some(Response::new().content("Invalid page"));
        }

        let Some((container, items)) = Self::get_page(&containers, page) else {
            return Some(Response::new().content("Invalid page"));
        };

        let details: Vec<_> = join_all(items.iter().map(|item| item.details(bot, extra))).await.into_iter().flatten().collect();

        Some(Self::build_embed(bot, container, &details, page, page_count, extra).await)
    }

    async fn try_handle_interaction(bot: &Bot, ctx: &Context, interaction: &ComponentInteraction, id: &str, extra: ExtraPageDetails) -> bool {
        let Some((_, rest)) = id.split_once("page-") else {
            return false;
        };

        if let Ok(new_page) = rest.parse::<usize>() {
            let Some(response) = Self::page(bot, extra, new_page).await else {
                interaction.simple_reply(ctx, "There was an error handling your interraction").await;
                return true;
            };

            let _ = interaction.create_response(ctx, CreateInteractionResponse::UpdateMessage(response.into())).await;
        } else {
            interaction.simple_reply(ctx, "Malformed page number").await;
        }

        true
    }
}

pub trait PageContainer<Item: PageItem<Details, ExtraPageDetails>, Details: PageDetails, ExtraPageDetails = ()> {
    fn items(&self) -> &[Item];
    fn page_setup(&self) -> CreateEmbed {
        CreateEmbed::new()
    }
}

pub trait PageItem<Details: PageDetails, ExtraPageDetails = ()> {
    async fn details(&self, bot: &Bot, extra: ExtraPageDetails) -> Option<Details>;
}

pub trait PageDetails {
    fn field(&self) -> (String, String, bool);
}

pub struct Response {
    content: Option<String>,
    embeds: Vec<CreateEmbed>,
    components: Vec<CreateActionRow>,
}

impl Response {
    fn new() -> Self {
        Response {
            content: None,
            embeds: vec![],
            components: vec![],
        }
    }

    fn content(mut self, content: impl Into<String>) -> Self {
        self.content = Some(content.into());
        self
    }

    fn add_embed(mut self, embed: CreateEmbed) -> Self {
        self.embeds.push(embed);
        self
    }

    fn components(mut self, components: Vec<CreateActionRow>) -> Self {
        self.components = components;
        self
    }
}

impl From<Response> for CreateInteractionResponseMessage {
    fn from(value: Response) -> Self {
        if let Some(content) = value.content {
            CreateInteractionResponseMessage::new()
                .ephemeral(true)
                .content(content)
                .embeds(value.embeds)
                .components(value.components)
        } else {
            CreateInteractionResponseMessage::new().ephemeral(true).embeds(value.embeds).components(value.components)
        }
    }
}

impl From<Response> for EditInteractionResponse {
    fn from(value: Response) -> Self {
        if let Some(content) = value.content {
            EditInteractionResponse::new().content(content).embeds(value.embeds).components(value.components)
        } else {
            EditInteractionResponse::new().embeds(value.embeds).components(value.components)
        }
    }
}
