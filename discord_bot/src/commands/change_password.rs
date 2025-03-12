use std::sync::Arc;

use core::error::Error as SpectreError;
use spectre_wallet_core::prelude::Secret;

use crate::utils::*;
use crate::TipContext;

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::ApplicationContext<'a, Arc<TipContext>, Error>;

#[poise::command(slash_command, category = "wallet")]
/// change wallet password
pub async fn change_password(
    ctx: Context<'_>,
    #[min_length = 10]
    #[description = "Old password"]
    old_password: String,
    #[min_length = 10]
    #[description = "New password"]
    new_password: String,
) -> Result<(), Error> {
    let user = ctx.author().id;
    let wallet_owner_identifier = user.to_string();

    let tip_context = ctx.data();

    if !tip_context.does_opened_owned_wallet_exists(&wallet_owner_identifier) {
        let embed = create_error_embed(
            "Error while changing the wallet password",
            "Wallet is not opened.",
        );
        return send_reply(ctx, embed, true).await;
    }

    let tip_wallet = tip_context
        .get_opened_owned_wallet(&wallet_owner_identifier)
        .ok_or("Wallet not found")?;

    // change secret
    match tip_wallet
        .change_secret(&Secret::from(old_password), &Secret::from(new_password))
        .await
    {
        Ok(_) => {
            let embed = create_success_embed("Success", "Password changed successfully.");
            send_reply(ctx, embed, true).await
        }
        Err(SpectreError::WalletError(spectre_wallet_core::error::Error::WalletDecrypt(_))) => {
            let embed = create_error_embed(
                "Error while changing the wallet password",
                "Old password is incorrect",
            );
            send_reply(ctx, embed, true).await
        }
        Err(error) => Err(Error::from(error)),
    }
}
