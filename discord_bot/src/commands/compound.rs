use std::sync::Arc;

use core::tip_context::TipContext;
use spectre_wallet_keys::secret::Secret;
use workflow_core::abortable::Abortable;

use crate::utils::*;

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::ApplicationContext<'a, Arc<TipContext>, Error>;

#[poise::command(slash_command, category = "wallet")]
/// compound utxo
pub async fn compound(
    ctx: Context<'_>,
    #[min_length = 10]
    #[description = "password"]
    password: String,
) -> Result<(), Error> {
    let author = ctx.author();
    let wallet_owner_identifier = author.id.to_string();

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
        let embed = create_error_embed("Error", "Wallet not initiated yet");
        return send_reply(ctx, embed, true).await;
    }

    if !is_opened {
        let embed = create_error_embed("Error", "Wallet not opened");
        return send_reply(ctx, embed, true).await;
    }

    let tip_wallet = match tip_context.get_opened_owned_wallet(&wallet_owner_identifier) {
        Some(w) => w,
        None => {
            let embed = create_error_embed("Error", "Unexpected error: wallet not opened");
            return send_reply(ctx, embed, true).await;
        }
    };

    let wallet = tip_wallet.wallet();

    let abortable = Abortable::default();
    let wallet_secret = Secret::from(password);

    let embed = create_warning_embed(
        "Compound in progress",
        "This may take a while, you can track progress using /wallet status command",
    );
    send_reply(ctx, embed, true).await?;

    let compound_result = wallet
        .account()?
        .sweep(wallet_secret, None, &abortable, None)
        .await;

    match compound_result {
        Err(error) => {
            let embed = create_error_embed("Error while compounding", &error.to_string());
            send_reply(ctx, embed, true).await?;
        }
        Ok(_) => {
            let embed = create_success_embed(
                "Compound completed",
                "Compounding is completed, existing UTXO's have been merged into one",
            );
            send_reply(ctx, embed, true).await?;
        }
    }

    return Ok(());
}
