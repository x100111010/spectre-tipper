use crate::{Context, Error};
use poise::{
    serenity_prelude::{Colour, CreateEmbed},
    CreateReply,
};

// embed creation
pub fn create_embed(title: &str, description: &str, colour: Colour) -> CreateEmbed {
    CreateEmbed::new()
        .title(title)
        .description(description)
        .colour(colour)
}

pub fn create_error_embed(title: &str, description: &str) -> CreateEmbed {
    create_embed(title, description, Colour::DARK_RED)
}

pub fn create_success_embed(title: &str, description: &str) -> CreateEmbed {
    create_embed(title, description, Colour::DARK_GREEN)
}

pub fn create_warning_embed(title: &str, description: &str) -> CreateEmbed {
    create_embed(title, description, Colour::ORANGE)
}

pub async fn send_reply(
    ctx: Context<'_>,
    embed: CreateEmbed,
    ephemeral: bool,
) -> Result<(), Error> {
    ctx.send(CreateReply {
        reply: false,
        embeds: vec![embed],
        ephemeral: Some(ephemeral),
        ..Default::default()
    })
    .await?;
    Ok(())
}
