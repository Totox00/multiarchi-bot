use serenity::all::{CommandInteraction, ComponentInteraction, Context, CreateInteractionResponse, CreateInteractionResponseMessage};

pub trait SimpleReply {
    async fn simple_reply<T: Into<String>>(&self, _ctx: &Context, _content: T);
}

impl SimpleReply for CommandInteraction {
    async fn simple_reply<T: Into<String>>(&self, ctx: &Context, content: T) {
        let _ = self
            .create_response(&ctx.http, CreateInteractionResponse::Message(CreateInteractionResponseMessage::new().ephemeral(true).content(content)))
            .await;
    }
}

impl SimpleReply for ComponentInteraction {
    async fn simple_reply<T: Into<String>>(&self, ctx: &Context, content: T) {
        let _ = self
            .create_response(&ctx.http, CreateInteractionResponse::Message(CreateInteractionResponseMessage::new().ephemeral(true).content(content)))
            .await;
    }
}

pub fn get_page<T, S, M: Fn(&T) -> &[S]>(item: &[T], r#fn: M, mut page: usize) -> Option<(&T, &[S])> {
    let mut item_iter = item.iter();

    let mut current_item = item_iter.next()?;
    let mut current_start = 0;

    while page != 0 {
        if r#fn(current_item).len() <= 25 || r#fn(current_item).len() - current_start <= 20 {
            current_item = item_iter.next()?;
        } else {
            current_start += 20;
        }

        page -= 1;
    }

    if r#fn(current_item).len() <= 25 {
        Some((current_item, r#fn(current_item)))
    } else {
        let end = r#fn(current_item).len().min(current_start + 20);
        Some((current_item, &r#fn(current_item)[current_start..end]))
    }
}
