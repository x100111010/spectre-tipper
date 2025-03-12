use crate::utils::*;
use core::tip_owned_wallet::TipOwnedWallet;
use spectre_wallet_core::prelude::{Language, Mnemonic};
use spectre_wallet_keys::secret::Secret;

use crate::models::{Context, Error};

#[poise::command(slash_command)]
/// restore (bip32) wallet from the mnemonic protected by a password of your choice
pub async fn restore(
    ctx: Context<'_>,
    #[description = "mnemonic"] mnemonic_phrase: String,
    #[min_length = 10]
    #[description = "new password"]
    password: String,
) -> Result<(), Error> {
    let mnemonic = match Mnemonic::new(mnemonic_phrase.trim(), Language::English) {
        Ok(mnemonic) => {
            // is a valid BIP32 mnemonic (12 or 24 words)
            let word_count = mnemonic.phrase().split_whitespace().count();
            if word_count != 12 && word_count != 24 {
                let embed = create_error_embed(
                    "Error while restoring the wallet",
                    "Mnemonic must be 12 or 24 words",
                );
                return send_reply(ctx, embed, true).await;
            }
            mnemonic
        }
        Err(_) => {
            let embed = create_error_embed(
                "Error while restoring the wallet",
                "Invalid mnemonic phrase",
            );
            return send_reply(ctx, embed, true).await;
        }
    };

    let user = ctx.author().id;
    let wallet_owner_identifier = user.to_string();

    let tip_context = ctx.data();

    let recovered_tip_wallet_result = TipOwnedWallet::restore(
        tip_context.clone(),
        &Secret::from(password),
        mnemonic,
        &wallet_owner_identifier,
    )
    .await?;

    let embed = create_success_embed(
        "Wallet Restored Successfully",
        "Your wallet has been restored from the mnemonic phrase",
    )
    .field(
        "Receive Address",
        recovered_tip_wallet_result.receive_address().to_string(),
        false,
    );

    send_reply(ctx, embed, true).await
}
