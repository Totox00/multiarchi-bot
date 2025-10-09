use serenity::all::{ChannelId, Context, Guild, GuildChannel, GuildId};

use crate::Bot;

const STATUS_CHANNEL: u64 = 1425955553976254474;
const MULTIARCHI_GUILD: u64 = 1342189623757242439;
const SYSTEM_CHANNEL: u64 = 1420513532247674901;
const CLAIMS_CHANNEL: u64 = 1342191337998516328;
const PRECLAIMS_CHANNEL: u64 = 1342191316318162967;

impl Bot {
    pub async fn status_channel(ctx: &Context) -> Option<GuildChannel> {
        let guild = Guild::get(ctx, GuildId::new(MULTIARCHI_GUILD)).await.ok()?;
        let mut channels = guild.channels(&ctx.http).await.ok()?;
        channels.remove(&ChannelId::new(STATUS_CHANNEL))
    }

    pub async fn system_channel(ctx: &Context) -> Option<GuildChannel> {
        let guild = Guild::get(ctx, GuildId::new(MULTIARCHI_GUILD)).await.ok()?;
        let mut channels = guild.channels(&ctx.http).await.ok()?;
        channels.remove(&ChannelId::new(SYSTEM_CHANNEL))
    }

    pub async fn claims_channel(ctx: &Context) -> Option<GuildChannel> {
        let guild = Guild::get(ctx, GuildId::new(MULTIARCHI_GUILD)).await.ok()?;
        let mut channels = guild.channels(&ctx.http).await.ok()?;
        channels.remove(&ChannelId::new(CLAIMS_CHANNEL))
    }

    pub async fn preclaims_channel(ctx: &Context) -> Option<GuildChannel> {
        let guild = Guild::get(ctx, GuildId::new(MULTIARCHI_GUILD)).await.ok()?;
        let mut channels = guild.channels(&ctx.http).await.ok()?;
        channels.remove(&ChannelId::new(PRECLAIMS_CHANNEL))
    }
}
