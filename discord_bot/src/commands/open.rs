use crate::utils::*;
use core::{error::Error as SpectreError, tip_owned_wallet::TipOwnedWallet};
use spectre_wallet_keys::secret::Secret;

use crate::models::{Context, Error};

#[poise::command(slash_command, category = "wallet")]
/// open the discord wallet using the password you defined
pub async fn open(
    ctx: Context<'_>,
    #[min_length = 10]
    #[description = "password"]
    password: String,
) -> Result<(), Error> {
    let user = ctx.author().id;
    let wallet_owner_identifier = user.to_string();

    let tip_context = ctx.data();

    // already opened
    if let Some(wallet) = tip_context.get_opened_owned_wallet(&wallet_owner_identifier) {
        let embed = create_success_embed(
            "Wallet Already Opened",
            &format!("Your wallet address: {}", wallet.receive_address()),
        );
        return send_reply(ctx, embed, true).await;
    }

    let tip_wallet_result = TipOwnedWallet::open(
        tip_context.clone(),
        &Secret::from(password),
        &wallet_owner_identifier,
    )
    .await;

    let tip_wallet = match tip_wallet_result {
        Ok(t) => t,
        Err(SpectreError::WalletError(spectre_wallet_core::error::Error::WalletDecrypt(_))) => {
            let embed = create_error_embed("Error while opening the wallet", "Password is wrong");
            return send_reply(ctx, embed, true).await;
        }
        Err(error) => return Err(Error::from(error)),
    };

    let embed = create_success_embed(
        "Wallet Opened Successfully",
        &format!("Your wallet address: {}", tip_wallet.receive_address()),
    );
    send_reply(ctx, embed, true).await
}
