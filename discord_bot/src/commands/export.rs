use std::sync::Arc;

use core::{tip_context::TipContext, tip_owned_wallet::TipOwnedWallet};

use crate::utils::*;

use spectre_wallet_keys::secret::Secret;

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::ApplicationContext<'a, Arc<TipContext>, Error>;

#[poise::command(slash_command, category = "wallet")]
/// export mnemonic and xpub
pub async fn export(
    ctx: Context<'_>,
    #[min_length = 10]
    #[description = "password"]
    password: String,
) -> Result<(), Error> {
    let user = ctx.author().id;
    let wallet_owner_identifier = user.to_string();

    let tip_context = ctx.data();

    let wallet_exists = tip_context
        .local_store()?
        .exists(Some(&wallet_owner_identifier))
        .await?;

    if !wallet_exists {
        let embed = create_error_embed("Error", "Wallet not found.");
        return send_reply(ctx, embed, true).await;
    }

    let tip_wallet = TipOwnedWallet::open(
        tip_context.clone(),
        &Secret::from(password.clone()),
        &wallet_owner_identifier,
    )
    .await?;

    let (mnemonic, xpub) = tip_wallet
        .export_mnemonic_and_xpub(&Secret::from(password))
        .await?;

    if let Some(mnemonic) = mnemonic {
        let embed = create_success_embed("Wallet Export", "")
            .field("Mnemonic Phrase", mnemonic.phrase(), false)
            .field("Extended Public Key (xpub)", xpub, false);

        send_reply(ctx, embed, true).await?;
    }

    Ok(())
}
