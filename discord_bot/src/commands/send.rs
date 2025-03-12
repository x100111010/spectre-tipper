use crate::utils::*;
use core::{
    error::Error as SpectreError,
    tip_transition_wallet::TipTransitionWallet,
    utils::{get_tx_explorer_url, try_parse_required_nonzero_spectre_as_sompi_u64},
};
use poise::{
    serenity_prelude::{self as serenity, CreateMessage},
    CreateReply,
};
use spectre_wallet_core::tx::{Fees, PaymentOutputs};
use spectre_wallet_keys::secret::Secret;
use std::sync::Arc;

use workflow_core::abortable::Abortable;

use crate::models::{Context, Error};

#[poise::command(slash_command, category = "wallet")]
/// send to user the given amount
pub async fn send(
    ctx: Context<'_>,
    #[description = "Send to"] user: serenity::User,
    #[description = "Amount"] amount: String,
    #[min_length = 10]
    #[description = "password"]
    password: String,
) -> Result<(), Error> {
    if user.bot || user.system {
        let embed = create_error_embed("Error", "User is a bot or a system user");
        return send_reply(ctx, embed, true).await;
    }

    let recipient_identifier = user.id.to_string();

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

    let amount_sompi = try_parse_required_nonzero_spectre_as_sompi_u64(Some(amount))?;
    println!("amount sompi {}", amount_sompi);

    let wallet = tip_wallet.wallet();

    // find address of recipient or create a temporary wallet
    let existing_owned_wallet = tip_context
        .owned_wallet_metadata_store
        .find_owned_wallet_metadata_by_owner_identifier(&recipient_identifier)
        .await;

    let recipient_address = match existing_owned_wallet {
        Ok(wallet) => wallet.receive_address,
        Err(SpectreError::OwnedWalletNotFound()) => {
            // find or create a temporary wallet
            let transition_wallet_result = tip_context
                .transition_wallet_metadata_store
                .find_transition_wallet_metadata_by_identifier_couple(
                    &author.id.to_string(),
                    &recipient_identifier,
                )
                .await?;

            match transition_wallet_result {
                Some(wallet) => wallet.receive_address,
                None => TipTransitionWallet::create(
                    tip_context.clone(),
                    &author.id.to_string(),
                    &recipient_identifier,
                )
                .await?
                .receive_address(),
            }
        }
        Err(e) => {
            let embed = create_error_embed("Error", &format!("Error: {:}", e));
            return send_reply(ctx, embed, true).await;
        }
    };

    let address = recipient_address;

    let outputs = PaymentOutputs::from((address, amount_sompi));
    let abortable = Abortable::default();
    let wallet_secret = Secret::from(password);

    let account = wallet.account()?;

    let (summary, hashes) = match account
        .send(
            outputs.into(),
            Fees::SenderPays(0),
            None,
            wallet_secret,
            None,
            &abortable,
            Some(Arc::new(
                move |ptx: &spectre_wallet_core::tx::PendingTransaction| {
                    println!("tx notifier: {:?}", ptx);
                },
            )),
        )
        .await
    {
        Ok(result) => result,
        Err(e) => {
            let embed = create_error_embed("Error", &format!("Transaction failed: {}", e));
            return send_reply(ctx, embed, true).await;
        }
    };

    let tx_id = hashes[0].to_string();

    let embed = create_success_embed(
        "Transaction Successful",
        &format!("<@{}> sent <@{}>: {}", author.id, user.id, summary),
    )
    .field("Txid", format!("{:?}", tx_id.clone()), false)
    .field(
        "Explorer",
        get_tx_explorer_url(&tx_id, tip_context.network_id().network_type()),
        false,
    );

    // public mentionning
    let public_message = CreateMessage::new()
        .add_embeds(vec![embed])
        .content(format!("<@{}>", user.id));
    ctx.channel_id().send_message(ctx, public_message).await?;

    // private mentionning
    ctx.send(CreateReply {
        content: Some("Transaction Successful".into()),
        ephemeral: Some(true),
        ..Default::default()
    })
    .await?;

    Ok(())
}
