use crate::utils::*;
use core::tip_owned_wallet::TipOwnedWallet;
use spectre_wallet_keys::secret::Secret;

use crate::models::{Context, Error};

#[poise::command(slash_command, category = "wallet")]
/// create (initiate) a fresh discord wallet protected by a password of your choice
pub async fn create(
    ctx: Context<'_>,
    #[min_length = 10]
    #[description = "password"]
    password: String,
) -> Result<(), Error> {
    let user = ctx.author().id;
    let wallet_owner_identifier = user.to_string();

    let tip_context = ctx.data();

    let is_opened = tip_context.does_opened_owned_wallet_exists(&wallet_owner_identifier);
    let is_initiated = match is_opened {
        true => true,
        false => {
            tip_context
                .local_store()?
                .exists(Some(&wallet_owner_identifier))
                .await?
        }
    };

    if is_initiated {
        let embed = create_error_embed("Error", "A discord wallet already exists");
        return send_reply(ctx, embed, true).await;
    }

    let (tip_wallet, mnemonic) = TipOwnedWallet::create(
        tip_context.clone(),
        &Secret::from(password),
        &wallet_owner_identifier,
    )
    .await?;

    let embed = create_success_embed("Wallet Created Successfully", "")
        .field("Mnemonic Phrase", mnemonic.phrase(), false)
        .field("Receive Address", tip_wallet.receive_address(), false);

    send_reply(ctx, embed, true).await
}
