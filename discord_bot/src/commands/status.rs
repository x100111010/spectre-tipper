use core::tip_transition_wallet::TipTransitionWallet;
use futures::future::join_all;

use crate::utils::*;
use spectre_wallet_core::utils::sompi_to_spectre_string_with_suffix;
use spectre_wallet_keys::secret::Secret;

use crate::models::{Context, Error};

#[poise::command(slash_command, category = "wallet")]
/// get the status of your discord wallet
pub async fn status(ctx: Context<'_>) -> Result<(), Error> {
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
            "Wallet Status",
            "The wallet has not been created yet. Use the `create` command to create a wallet.",
        );
        return send_reply(ctx, embed, true).await;
    }

    if !is_opened {
        let embed = create_error_embed(
            "Wallet Status",
            "The wallet is not opened. Use the `open` command to open the wallet and display its balance.",
        );
        return send_reply(ctx, embed, true).await;
    }

    let owned_wallet = tip_context
        .get_opened_owned_wallet(&wallet_owner_identifier)
        .unwrap();

    let account = owned_wallet.wallet().account().unwrap();

    let balance = account.balance().unwrap_or_default();

    let transition_wallets = tip_context
        .transition_wallet_metadata_store
        .find_transition_wallet_metadata_by_target_identifier(&wallet_owner_identifier)
        .await?;

    let pending_transition_balance = join_all(transition_wallets.iter().map(|metadata| async {
        let secret = Secret::from(metadata.secret.clone());
        let transition_wallet = TipTransitionWallet::open(
            tip_context.clone(),
            &secret,
            &metadata.initiator_identifier,
            &metadata.target_identifier,
        )
        .await;

        let balance: u64 = match transition_wallet {
            Ok(tw) => {
                let account_result = tw.wallet().account();
                let mut b = 0;
                if let Ok(account) = account_result {
                    if let Some(balance) = account.balance() {
                        b = balance.mature
                    }
                }

                b
            }
            Err(e) => {
                println!("warning: {:?}", e);

                0_u64
            }
        };

        balance
    }))
    .await
    .into_iter()
    .reduce(|a, b| a + b);

    let network_type = tip_context.network_id();
    let balance_formatted = sompi_to_spectre_string_with_suffix(balance.mature, &network_type);
    let pending_balance_formatted =
        sompi_to_spectre_string_with_suffix(balance.pending, &network_type);

    let pending_transition_balance_formatted =
        sompi_to_spectre_string_with_suffix(pending_transition_balance.unwrap_or(0), &network_type);

    let embed = create_success_embed("Wallet Status", "")
        .field("Balance", balance_formatted, true)
        .field("Pending Balance", pending_balance_formatted, true)
        .field("UTXO count", balance.mature_utxo_count.to_string(), true)
        .field(
            "Pending UTXO count",
            balance.pending_utxo_count.to_string(),
            true,
        )
        .field(
            "Balance to be claimed",
            pending_transition_balance_formatted,
            true,
        );

    send_reply(ctx, embed, true).await
}
