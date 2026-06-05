use async_trait::async_trait;
use eden_common::{
    domain::{Notifier, notifier::LinkedMcAccountLogin},
    minecraft::{HeadIconSource, McService},
};
use eden_config::LiveConfig;
use eden_model::{alerts::command::CommandAlert, tables::mc_login_event::McLoginEvent};
use eden_twilight::http::ResponseFutureExt;
use erased_report::ErasedReport;
use std::sync::Arc;
use twilight_util::builder::embed::{
    EmbedAuthorBuilder, EmbedBuilder, EmbedFieldBuilder, ImageSource,
};

use crate::context::BotContext;

#[derive(Debug)]
pub struct DiscordNotifier {
    config: LiveConfig,
    http: Arc<twilight_http::Client>,
}

impl DiscordNotifier {
    #[must_use]
    pub fn new(ctx: &BotContext) -> Arc<Self> {
        Arc::new(Self {
            config: ctx.bot_live_config.clone(),
            http: ctx.http.clone(),
        })
    }
}

#[allow(unused)]
#[async_trait]
impl Notifier for DiscordNotifier {
    async fn admin_used_command(&self, metadata: &CommandAlert) -> Result<(), ErasedReport> {
        let config = self.config.get();
        let minecraft = McService::new(config.clone());

        let Some(alert_channel_id) = config
            .organization
            .discord
            .as_ref()
            .and_then(|v| v.ids.alert_channel)
        else {
            return Ok(());
        };

        Ok(())
    }

    async fn guest_player_joined(&self, event: &McLoginEvent) -> Result<(), ErasedReport> {
        let config = self.config.get();
        let minecraft = McService::new(config.clone());

        let Some(alert_channel_id) = config
            .organization
            .discord
            .as_ref()
            .and_then(|v| v.ids.alert_channel)
        else {
            return Ok(());
        };

        let head_icon_url = minecraft.get_head_icon_url(HeadIconSource::Uuid(event.player_uuid));
        let embed_icon_url = ImageSource::url(head_icon_url)
            .expect("minecraft.get_head_icon_url should produce valid URL");

        let author_field = EmbedAuthorBuilder::new("Guest")
            .icon_url(embed_icon_url)
            .build();

        let embed = EmbedBuilder::new()
            .author(author_field)
            .field(EmbedFieldBuilder::new("Event ID", format!("`{}`", event.id)).inline())
            .field(EmbedFieldBuilder::new("IP Address", format!("`{}`", event.ip_address)).inline())
            .field(EmbedFieldBuilder::new("Edition", format!("`{:?}`", event.edition)).inline())
            .timestamp(event.created_at.into_twilight())
            .build();

        self.http
            .create_message(alert_channel_id)
            .content("**A guest player joined the server!**")
            .embeds(&[embed])
            .perform()
            .await?;

        Ok(())
    }

    async fn revoked_login(&self, _metadata: &LinkedMcAccountLogin) -> Result<(), ErasedReport> {
        Ok(())
    }
}
