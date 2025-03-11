use std::sync::Arc;

use crate::utils::*;
use core::tip_context::TipContext;

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::ApplicationContext<'a, Arc<TipContext>, Error>;

#[poise::command(slash_command, category = "wallet")]
/// close the opened discord wallet
pub async fn close(ctx: Context<'_>) -> Result<(), Error> {
    let tip_context = ctx.data;

    let user = ctx.author().id;
    let wallet_owner_identifier = user.to_string();

    let is_opened = tip_context.does_opened_owned_wallet_exists(&wallet_owner_identifier);

    if is_opened {
        let tip_wallet_result = tip_context.remove_opened_owned_wallet(&wallet_owner_identifier);

        if let Some(tip_wallet) = tip_wallet_result {
            tip_wallet.wallet().stop().await?;
            tip_wallet.wallet().close().await?;
        }
    }

    let embed = create_success_embed("Wallet Closed", "Your wallet has been successfully closed.");
    send_reply(ctx, embed, true).await
}
