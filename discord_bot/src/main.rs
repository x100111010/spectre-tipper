use std::{env, path::Path, str::FromStr, sync::Arc, time::Duration};

use core::{
    error::Error as SpectreError, tip_context::TipContext, tip_owned_wallet::TipOwnedWallet,
    tip_transition_wallet::TipTransitionWallet,
    utils::try_parse_required_nonzero_spectre_as_sompi_u64,
};
use futures::future::join_all;
use poise::{
    serenity_prelude::{self as serenity, Colour, CreateEmbed, CreateMessage},
    CreateReply, Modal,
};
use tokio::fs;

use spectre_wallet_core::{
    prelude::{Address, Language, Mnemonic},
    rpc::ConnectOptions,
    settings::application_folder,
    tx::{Fees, PaymentOutputs},
    utils::sompi_to_spectre_string_with_suffix,
};
use spectre_wallet_keys::secret::Secret;
use spectre_wrpc_client::{
    prelude::{ConnectStrategy, GetServerInfoResponse, NetworkId, RpcApi},
    Resolver, SpectreRpcClient, WrpcEncoding,
};

use workflow_core::abortable::Abortable;

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::ApplicationContext<'a, Arc<TipContext>, Error>;

// embed creation
fn create_embed(title: &str, description: &str, colour: Colour) -> CreateEmbed {
    CreateEmbed::new()
        .title(title)
        .description(description)
        .colour(colour)
}

fn create_error_embed(title: &str, description: &str) -> CreateEmbed {
    create_embed(title, description, Colour::DARK_RED)
}

fn create_success_embed(title: &str, description: &str) -> CreateEmbed {
    create_embed(title, description, Colour::DARK_GREEN)
}

async fn send_reply(ctx: Context<'_>, embed: CreateEmbed, ephemeral: bool) -> Result<(), Error> {
    ctx.send(CreateReply {
        reply: false,
        embeds: vec![embed],
        ephemeral: Some(ephemeral),
        ..Default::default()
    })
    .await?;
    Ok(())
}

// TODO: move cmd to dedicated files

#[poise::command(
    slash_command,
    subcommands(
        "create",
        "open",
        "close",
        "restore",
        "export",
        "status",
        "destroy",
        "send",
        "claim",
        "change_secret",
        "withdraw"
    ),
    category = "wallet"
)]
/// Main command for interracting with the discord wallet
async fn wallet(_: Context<'_>) -> Result<(), Error> {
    Ok(())
}

#[poise::command(slash_command, category = "wallet")]
/// create (initiate) a fresh discord wallet with a secret
async fn create(
    ctx: Context<'_>,
    #[min_length = 10]
    #[description = "secret"]
    secret: String,
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
        &Secret::from(secret),
        &wallet_owner_identifier,
    )
    .await?;

    let embed = create_success_embed("Wallet Created Successfully", "")
        .field("Mnemonic Phrase", mnemonic.phrase(), false)
        .field("Receive Address", tip_wallet.receive_address(), false);

    send_reply(ctx, embed, true).await
}

#[poise::command(slash_command, category = "wallet")]
/// open the discord wallet using the secret
async fn open(
    ctx: Context<'_>,
    #[min_length = 10]
    #[description = "secret"]
    secret: String,
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
        &Secret::from(secret),
        &wallet_owner_identifier,
    )
    .await;

    let tip_wallet = match tip_wallet_result {
        Ok(t) => t,
        Err(SpectreError::WalletError(spectre_wallet_core::error::Error::WalletDecrypt(_))) => {
            let embed = create_error_embed("Error while opening the wallet", "Secret is wrong");
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

#[poise::command(slash_command, category = "wallet")]
/// close the opened discord wallet
async fn close(ctx: Context<'_>) -> Result<(), Error> {
    let tip_context = ctx.data;

    let user = ctx.author().id;
    let wallet_owner_identifier = user.to_string();

    let is_opened = tip_context.does_opened_owned_wallet_exists(&wallet_owner_identifier);

    if is_opened {
        let tip_wallet_result = tip_context.remove_opened_owned_wallet(&wallet_owner_identifier);

        if let Some(tip_wallet) = tip_wallet_result {
            tip_wallet.wallet().close().await?;
        }
    }

    let embed = create_success_embed("Wallet Closed", "Your wallet has been successfully closed.");
    send_reply(ctx, embed, true).await
}

#[poise::command(slash_command, category = "wallet")]
/// get the status of your discord wallet
async fn status(ctx: Context<'_>) -> Result<(), Error> {
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

    let balance: u64 = {
        let mut b: u64 = 0;
        if let Some(owned_wallet) = tip_context.get_opened_owned_wallet(&wallet_owner_identifier) {
            if let Ok(account) = owned_wallet.wallet().account() {
                if let Some(balance) = account.balance() {
                    b = balance.mature;
                }
            }
        }
        b
    };

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

                0 as u64
            }
        };

        return balance;
    }))
    .await
    .into_iter()
    .reduce(|a, b| a + b);

    let network_type = tip_context.network_id();
    let balance_formatted = sompi_to_spectre_string_with_suffix(balance, &network_type);
    let pending_transition_balance_formatted =
        sompi_to_spectre_string_with_suffix(pending_transition_balance.unwrap_or(0), &network_type);

    let embed = create_success_embed("Wallet Status", "")
        .field("Is Opened", is_opened.to_string(), true)
        .field("Is Initiated", is_initiated.to_string(), true)
        .field("Balance", balance_formatted, true)
        .field(
            "Pending Transition Balance",
            pending_transition_balance_formatted,
            true,
        );

    send_reply(ctx, embed, true).await
}

#[poise::command(slash_command, category = "wallet")]
/// transfers funds from transition_wallet to owned_wallet
async fn claim(ctx: Context<'_>) -> Result<(), Error> {
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

    if !is_opened && !is_initiated {
        let embed = create_error_embed(
            "Error",
            &format!(
                "Wallet is not opened or initiated.\nis_opened: {}\nis_initiated: {}",
                is_opened, is_initiated
            ),
        );
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
    let owner_receive_address = wallet.account().unwrap().receive_address().unwrap();

    let transition_wallets = tip_context
        .transition_wallet_metadata_store
        .find_transition_wallet_metadata_by_target_identifier(&wallet_owner_identifier)
        .await?;

    // check if there are any pending balances in transition wallets
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

                0 as u64
            }
        };

        return balance;
    }))
    .await
    .into_iter()
    .reduce(|a, b| a + b);

    if pending_transition_balance.unwrap_or(0) == 0 {
        let embed = create_error_embed(
            "No Funds to Claim",
            "No coins stored in the transition wallets, aborting.",
        );
        return send_reply(ctx, embed, true).await;
    }

    join_all(transition_wallets.iter().map(|metadata| async {
        let secret = Secret::from(metadata.secret.clone());
        let transition_wallet = TipTransitionWallet::open(
            tip_context.clone(),
            &secret,
            &metadata.initiator_identifier,
            &metadata.target_identifier,
        )
        .await;

        match transition_wallet {
            Ok(tw) => {
                let account_result = tw.wallet().account();
                if let Ok(account) = account_result {
                    if let Some(balance) = account.balance() {
                        let receive_address = owner_receive_address.clone();

                        println!(
                            "sending {} SPR from {} to {}",
                            balance.mature,
                            account.receive_address().unwrap().address_to_string(),
                            owner_receive_address.address_to_string()
                        );

                        let address = receive_address;

                        let amount_sompi = balance.mature;
                        // 10_000 is arbitrary and could/should be estimated before hand
                        // https://kaspa-mdbook.aspectron.com/transactions/constraints/fees.html
                        let amount_minus_gas_fee = amount_sompi - 10000;

                        println!(
                            "amount sompi {}, with gas fee {}",
                            amount_sompi, amount_minus_gas_fee
                        );

                        let outputs = PaymentOutputs::from((address, amount_minus_gas_fee));
                        let abortable = Abortable::default();
                        let wallet_secret = Secret::from(secret);

                        let (summary, hashes) = account
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
                            .unwrap();

                        println!("summary {:?}, hashes: {:?}", summary, hashes);

                        let embed = create_success_embed(
                            "Successfully claimed funds from transition wallets.",
                            &format!(
                                "is opened: {}\nis initiated: {}\n summary {:?}\n hashes: {:?}",
                                is_opened, is_initiated, summary, hashes
                            ),
                        );
                        send_reply(ctx.clone(), embed, true).await?;
                    }
                }
            }
            Err(e) => {
                println!("warning: {:?}", e);
            }
        };

        Ok::<(), Error>(())
    }))
    .await;

    Ok(())
}

#[derive(Debug, poise::Modal)]
#[name = "Confirm wallet destruction"]
struct DestructionModalConfirmation {
    #[name = "write destroy to confirm"]
    first_input: String,
}

#[poise::command(slash_command, category = "wallet")]
/// destroy your existing (if exists) discord wallet
async fn destroy(ctx: Context<'_>) -> Result<(), Error> {
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

#[poise::command(slash_command)]
/// restore (bip32) wallet from the mnemonic
async fn restore(
    ctx: Context<'_>,
    #[description = "mnemonic"] mnemonic_phrase: String,
    #[min_length = 10]
    #[description = "new secret"]
    secret: String,
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
        &Secret::from(secret),
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

#[poise::command(slash_command, category = "wallet")]
/// export mnemonic and xpub
async fn export(
    ctx: Context<'_>,
    #[min_length = 10]
    #[description = "secret"]
    secret: String,
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
        &Secret::from(secret.clone()),
        &wallet_owner_identifier,
    )
    .await?;

    let (mnemonic, xpub) = tip_wallet
        .export_mnemonic_and_xpub(&Secret::from(secret))
        .await?;

    if let Some(mnemonic) = mnemonic {
        let embed = create_success_embed("Wallet Export", "")
            .field("Mnemonic Phrase", mnemonic.phrase(), false)
            .field("Extended Public Key (xpub)", xpub, false);

        send_reply(ctx, embed, true).await?;
    }

    Ok(())
}

#[poise::command(slash_command, category = "wallet")]
/// change secret
async fn change_secret(
    ctx: Context<'_>,
    #[min_length = 10]
    #[description = "Old secret"]
    old_secret: String,
    #[min_length = 10]
    #[description = "New secret"]
    new_secret: String,
) -> Result<(), Error> {
    let user = ctx.author().id;
    let wallet_owner_identifier = user.to_string();

    let tip_context = ctx.data();

    if !tip_context.does_opened_owned_wallet_exists(&wallet_owner_identifier) {
        let embed = create_error_embed(
            "Error while changing the wallet secret",
            "Wallet is not opened.",
        );
        return send_reply(ctx, embed, true).await;
    }

    let tip_wallet = tip_context
        .get_opened_owned_wallet(&wallet_owner_identifier)
        .ok_or("Wallet not found")?;

    // change secret
    match tip_wallet
        .change_secret(&Secret::from(old_secret), &Secret::from(new_secret))
        .await
    {
        Ok(_) => {
            let embed = create_success_embed("Success", "Secret changed successfully.");
            send_reply(ctx, embed, true).await
        }
        Err(SpectreError::WalletError(spectre_wallet_core::error::Error::WalletDecrypt(_))) => {
            let embed = create_error_embed(
                "Error while changing the wallet secret",
                "Old secret is incorrect",
            );
            send_reply(ctx, embed, true).await
        }
        Err(error) => return Err(Error::from(error)),
    }
}

#[poise::command(slash_command, category = "wallet")]
/// send to user the given amount
async fn send(
    ctx: Context<'_>,
    #[description = "Send to"] user: serenity::User,
    #[description = "Amount"] amount: String,
    #[min_length = 10]
    #[description = "Wallet secret"]
    secret: String,
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

    let amount_sompi = try_parse_required_nonzero_spectre_as_sompi_u64(Some(amount))?;
    println!("amount sompi {}", amount_sompi);

    let outputs = PaymentOutputs::from((address, amount_sompi));
    let abortable = Abortable::default();
    let wallet_secret = Secret::from(secret);

    let account = wallet.account()?;

    let (summary, hashes) = match account
        .send(
            outputs.into(),
            i64::from(0).into(),
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

    let embed = create_success_embed(
        "Transaction Successful",
        &format!("<@{}> sent <@{}>: {}", author.id, user.id, summary),
    )
    .field("Txid", format!("{:?}", hashes), false);

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

#[poise::command(slash_command, category = "wallet")]
/// withdraw funds to a custom Spectre address
async fn withdraw(
    ctx: Context<'_>,
    #[description = "Spectre wallet address"] address: String,
    #[description = "Amount"] amount: String,
    #[min_length = 10]
    #[description = "Wallet secret"]
    secret: String,
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

    let wallet = tip_wallet.wallet();

    let outputs = PaymentOutputs::from((recipient_address.clone(), amount_sompi));
    let abortable = Abortable::default();
    let wallet_secret = Secret::from(secret);

    let account = wallet.account()?;

    let (summary, hashes) = match account
        .send(
            outputs.into(),
            i64::from(0).into(),
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

    let embed = create_success_embed(
        "Withdrawal Successful",
        &format!("Withdrew to address `{}`: {}", recipient_address, summary),
    )
    .field("Txid", format!("{:?}", hashes), false);

    send_reply(ctx, embed, true).await
}

#[tokio::main]
async fn main() {
    // load local .env or ignore if file doesn't exists
    match dotenvy::dotenv() {
        Ok(_) => println!("Environment variables loaded from .env"),
        Err(_) => println!("Not loading environement variables from .env"),
    }

    let discord_token = match env::var("DISCORD_TOKEN") {
        Ok(v) => v,
        Err(_) => panic!("DISCORD_TOKEN environment variable is missing."),
    };

    let spectre_network_str =
        env::var("SPECTRE_NETWORK").expect("SPECTRE_NETWORK environment variable is missing");

    let wallet_data_path_str =
        env::var("WALLET_DATA_PATH").expect("WALLET_DATA_PATH environment variable is missing");

    // RPC
    let forced_spectre_node: Option<String> = match env::var("FORCE_SPECTRE_NODE_ADDRESS") {
        Ok(v) => Some(v),
        Err(_) => None,
    };

    let resolver = match forced_spectre_node.clone() {
        Some(value) => Resolver::new(Some(vec![Arc::new(value)]), true), // tls
        _ => Resolver::default(),
    };

    let network_id = NetworkId::from_str(&spectre_network_str).unwrap();

    let wrpc_client = Arc::new(
        SpectreRpcClient::new(
            WrpcEncoding::Borsh,
            forced_spectre_node.as_deref(),
            Some(resolver.clone()),
            Some(network_id.clone()),
            None,
        )
        .unwrap(),
    );

    let connect_timeout = Duration::from_secs(5);

    match wrpc_client
        .connect(Some(ConnectOptions {
            url: forced_spectre_node.clone(),
            block_async_connect: true,
            connect_timeout: Some(connect_timeout),
            strategy: ConnectStrategy::Fallback,
            ..Default::default()
        }))
        .await
    {
        Ok(_) => println!("Successfully connected to the node."),
        Err(e) => {
            eprintln!("Failed to connect to the node: {}", e);
            panic!("Connection failed: {}", e);
        }
    }

    match check_node_status(&wrpc_client).await {
        Ok(_) => {
            println!("Successfully completed client connection to the Spectre node!");
        }
        Err(error) => {
            eprintln!("An error occurred: {}", error);
            std::process::exit(1);
        }
    }

    let wallet_data_path_buf = Path::new(&wallet_data_path_str).to_path_buf();

    let tip_context = TipContext::try_new_arc(
        resolver,
        NetworkId::from_str(&spectre_network_str).unwrap(),
        forced_spectre_node,
        wrpc_client,
        wallet_data_path_buf,
    );

    if let Err(e) = tip_context {
        panic!("{}", format!("Error while building tip context: {}", e));
    }

    // discord
    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![wallet()],
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(tip_context.unwrap())
            })
        })
        .build();

    let intents = serenity::GatewayIntents::non_privileged();
    let client = serenity::ClientBuilder::new(discord_token, intents)
        .framework(framework)
        .await;
    client.unwrap().start().await.unwrap();
}

async fn check_node_status(
    wrpc_client: &Arc<SpectreRpcClient>,
) -> Result<(), Box<dyn std::error::Error>> {
    let GetServerInfoResponse {
        is_synced,
        server_version,
        network_id,
        has_utxo_index,
        ..
    } = wrpc_client.get_server_info().await?;

    println!("Node version: {}", server_version);
    println!("Network: {}", network_id);
    println!("is synced: {}", is_synced);
    println!("is indexing UTXOs: {}", has_utxo_index);

    if is_synced {
        Ok(())
    } else {
        Err("Node is not synced".into())
    }
}
