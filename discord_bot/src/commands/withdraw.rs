use core::utils::{
    estimate_fees, get_tx_explorer_url, try_parse_required_nonzero_spectre_as_sompi_u64,
};
use spectre_wallet_core::{
    prelude::Address,
    tx::{Fees, PaymentOutputs},
};
use spectre_wallet_keys::secret::Secret;
use std::sync::Arc;
use workflow_core::abortable::Abortable;

use crate::utils::*;

use crate::models::{Context, Error};

#[poise::command(slash_command, category = "wallet")]
/// withdraw funds to a custom Spectre address
pub async fn withdraw(
    ctx: Context<'_>,
    #[description = "Spectre wallet address"] address: String,
    #[description = "Amount"] amount: String,
    #[min_length = 10]
    #[description = "password"]
    password: String,
) -> Result<(), Error> {
    let recipient_address = match Address::try_from(address.as_str()) {
        Ok(address) => address,
        Err(_) => {
            let embed =
                create_error_embed("Error while withdrawing funds", "Invalid Spectre address");
            return send_reply(ctx, embed, true).await;
        }
    };

    let amount_sompi = try_parse_required_nonzero_spectre_as_sompi_u64(Some(amount))?;

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

    ctx.defer_ephemeral().await?;

    let wallet = tip_wallet.wallet();
    let account = wallet.account()?;

    let generator_summary_option = estimate_fees(
        &account,
        PaymentOutputs::from((recipient_address.clone(), amount_sompi)),
    )
    .await?;

    let amount_minus_gas_fee = match generator_summary_option.final_transaction_amount {
        Some(final_transaction_amount) => final_transaction_amount,
        None => {
            let embed = create_error_embed(
                "Error",
                "While estimating the transaction fees, final_transaction_amount is None.",
            );
            return send_reply(ctx, embed, true).await;
        }
    };

    let abortable = Abortable::default();
    let wallet_secret = Secret::from(password);
    let outputs = PaymentOutputs::from((recipient_address.clone(), amount_minus_gas_fee));

    let (summary, hashes) = match account
        .send(
            outputs.into(),
            Fees::ReceiverPays(0),
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
            let embed = create_error_embed("Error", &format!("Withdrawal failed: {}", e));
            return send_reply(ctx, embed, true).await;
        }
    };

    let tx_id = hashes[0].to_string();

    let embed = create_success_embed(
        "Withdrawal Successful",
        &format!("Withdrew to address `{}`: {}", recipient_address, summary),
    )
    .field("Txid", format!("{:?}", tx_id.clone()), false)
    .field(
        "Explorer",
        get_tx_explorer_url(&tx_id, tip_context.network_id().network_type()),
        false,
    );

    send_reply(ctx, embed, true).await
}
