use anyhow::{bail, Context as AnyhowContext};
use serenity::{client::Context, model::channel::Message};

use crate::{
    auth::IsBlacklisted,
    responses::{create_request_log_message, delete_user_request, edit_request},
    structs::{Config, PendingRequestMidStore, PendingRequestUidStore},
};

pub async fn handle_user_request(ctx: Context, msg: Message) -> anyhow::Result<()> {
    if msg.author.is_blacklisted(&ctx).await? {
        bail!("User is blacklisted");
    };

    // check if user has an existing request, if so, cancel it first

    let mut data = ctx.data.write().await;
    let pending_request_uid_store = data
        .get_mut::<PendingRequestUidStore>()
        .context("Could not get pending request store")?;

    let message_id = pending_request_uid_store.remove(&msg.author.id);

    let pending_request_mid_store = data
        .get_mut::<PendingRequestMidStore>()
        .context("Could not get pending request store")?;

    pending_request_mid_store.remove(&msg.id);

    let config = data.get::<Config>().context("Could not get config")?;
    // let valid_image_types = config.settings.image_types.clone();

    match message_id {
        Some(message_id) => {
            let existing_request = config
                .server
                .log_channel_id
                .message(&ctx.http, message_id)
                .await;

            match existing_request {
                Ok(mut existing_request) => {
                    let embed = existing_request.embeds.first().context("No embed")?;
                    let embed_link = embed.url.clone().context("No message to delete")?;

                    let mut uid: Option<String> = None;

                    for field in &embed.fields {
                        match field.name.as_str() {
                            "UID" => {
                                uid = Some(field.value.clone());
                                break;
                            }
                            _ => {}
                        }
                    }

                    let uid: String = uid.context("Could not parse uid from embed")?;

                    let result = edit_request(
                        &ctx,
                        &mut existing_request,
                        "Request Cancelled",
                        None,
                        None,
                        false,
                    )
                    .await
                    .context("Could not edit request message");
                    if result.is_err() {
                        println!("{:?}", result);
                    }

                    // drop(data);

                    // delete_user_request(&ctx, uid, &embed_link)
                    //     .await
                    //     .context("Could not delete original request")?;
                }
                Err(_) => {}
            }
        }
        None => {}
    }

    // Get message attachment and create request

    // let data = ctx.data.write().await;
    // let config = data.get::<Config>().context("Could not get config")?;

    let message_attachment = msg.attachments.first().context("No message attachment")?;
    let attachment_content_type = &message_attachment
        .content_type
        .as_ref()
        .context("Could not get content-type")?[6..];

    if config.settings.image_types.contains(&attachment_content_type.to_string())
    {
        drop(data);

        let created_message_id = create_request_log_message(&ctx, &msg).await?; // Add error handling here (log to channel?)

        // Add new request to local store

        let mut data = ctx.data.write().await;
        let pending_request_store = data
            .get_mut::<PendingRequestUidStore>()
            .context("Could not get pending request store")?;
        pending_request_store.insert(msg.author.id, created_message_id);

        let pending_request_mid_store = data
            .get_mut::<PendingRequestMidStore>()
            .context("Could not get pending request store")?;
        pending_request_mid_store.insert(msg.id, created_message_id);
    }

    Ok(())
}
