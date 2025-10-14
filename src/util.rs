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
