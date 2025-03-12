mod commands;
mod utils;

use core::tip_context::TipContext;
use poise::{
    samples::on_error,
    serenity_prelude::{self as serenity},
    CreateReply, FrameworkError,
};
use spectre_wallet_core::rpc::ConnectOptions;
use spectre_wrpc_client::{
    prelude::{ConnectStrategy, GetServerInfoResponse, NetworkId, RpcApi},
    Resolver, SpectreRpcClient, WrpcEncoding,
};
use std::{env, path::Path, str::FromStr, sync::Arc, time::Duration};

use crate::commands::*;
use crate::utils::*;

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::ApplicationContext<'a, Arc<TipContext>, Error>;

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
        "change_password",
        "withdraw",
        "compound"
    ),
    category = "wallet"
)]
/// Main command for interracting with the discord wallet
async fn wallet(_: Context<'_>) -> Result<(), Error> {
    Ok(())
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
            Some(network_id),
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
            on_error: |error| {
                Box::pin(async move {
                    match error {
                        // set ephemeral to true by default on unexpected command error (avoid data leaks on unhandled errors)
                        FrameworkError::Command { ctx, error, .. } => {
                            let error = error.to_string();
                            eprintln!("An error occured in a command: {}", error);

                            let embed =
                                create_error_embed("Error", &format!("An unexpected error occured, please report bugs to the developers: {}", error));
                            let send_result = ctx.send(CreateReply {
                                reply: false,
                                embeds: vec![embed],
                                ephemeral: Some(true),
                                ..Default::default()
                            })
                            .await;

                            match send_result {
                                Ok(_) => (),
                                _ => eprintln!("Error - Impossible to forward error via Discord, initial error: {}", error),
                            };
                        }
                        // fallback all other error types to the default error handler
                        _ => {
                            if let Err(e) = on_error(error).await {
                                tracing::error!("Error while handling error: {}", e);
                            }
                        }
                    }
                })
            },
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
