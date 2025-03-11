use std::sync::Arc;

use crate::utils::*;
use core::tip_context::TipContext;
use poise::{serenity_prelude::Colour, Modal};
use spectre_wallet_core::settings::application_folder;
use tokio::fs;

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::ApplicationContext<'a, Arc<TipContext>, Error>;

#[derive(Debug, poise::Modal)]
#[name = "Confirm wallet destruction"]
struct DestructionModalConfirmation {
    #[name = "write destroy to confirm"]
    first_input: String,
}

#[poise::command(slash_command, category = "wallet")]
/// destroy your existing (if exists) discord wallet
pub async fn destroy(ctx: Context<'_>) -> Result<(), Error> {
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

    if !is_initiated {
        let embed = create_error_embed(
            "Error",
            "The wallet is not initiated, cannot destroy a non-existing wallet.",
        );
        return send_reply(ctx, embed, true).await;
    }

    let result = DestructionModalConfirmation::execute(ctx).await?;

    if let Some(data) = result {
        if data.first_input == "destroy" {
            if is_opened {
                let tip_wallet_result =
                    tip_context.remove_opened_owned_wallet(&wallet_owner_identifier);

                if let Some(tip_wallet) = tip_wallet_result {
                    tip_wallet.wallet().close().await?;
                };
            }

            // remove from store
            tip_context
                .owned_wallet_metadata_store
                .remove_by_owner_identifier(wallet_owner_identifier.clone())
                .await?;

            // delete wallet file
            let wallet_folder = application_folder()?;
            let wallet_file = wallet_folder.join(format!("{}.wallet", wallet_owner_identifier));

            if wallet_file.exists() {
                fs::remove_file(&wallet_file).await?;
            }

            let embed = create_success_embed("Wallet Destroyed", "");
            return send_reply(ctx, embed, true).await;
        }
    }

    let embed = create_embed("Wallet Destruction Aborted", "", Colour::DARK_ORANGE);
    send_reply(ctx, embed, true).await
}
