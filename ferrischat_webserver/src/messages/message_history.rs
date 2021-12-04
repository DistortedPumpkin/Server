use crate::WebServerError;
use axum::extract::{Path, Query};
use ferrischat_common::request_json::GetMessageHistoryParams;
use ferrischat_common::types::{BadRequestJson, Message, MessageHistory, User, UserFlags};
use serde::Serialize;

/// GET `/api/v0/channels/{channel_id}/messages`
pub async fn get_message_history(
    Path(channel_id): Path<u128>,
    _: crate::Authorization,
    Query(GetMessageHistoryParams {
        mut limit,
        oldest_first,
        mut offset,
    }): Query<GetMessageHistoryParams>,
) -> Result<crate::Json<MessageHistory>, WebServerError> {
    let bigint_channel_id = u128_to_bigdecimal!(channel_id);
    let db = get_db_or_fail!();

    let oldest_first = oldest_first.unwrap_or(false);

    if limit < Some(0) {
        return Err((
            400,
            BadRequestJson {
                reason: "limit must be > 0".to_string(),
                location: None,
            },
        )
            .into());
    }

    if offset < Some(0) {
        offset = Some(0);
    }

    let messages = if oldest_first {
        let mut resp = sqlx::query!(
            r#"
SELECT m.*,
       a.name AS author_name,
       a.flags AS author_flags,
       a.discriminator AS author_discriminator,
       a.pronouns AS author_pronouns
FROM messages m
    CROSS JOIN LATERAL (
        SELECT *
        FROM users
        WHERE id = m.author_id
        ) as a
WHERE channel_id = $1
ORDER BY id ASC
LIMIT $2 OFFSET $3
"#,
            bigint_channel_id,
            limit,
            offset,
        )
        .fetch_all(db)
        .await?;

        resp.iter_mut().map(|x| {
            (
                x.id,
                std::mem::take(&mut x.content),
                x.channel_id,
                x.author_id,
                std::mem::take(&mut x.author_name),
                x.author_flags,
                x.author_discriminator,
                x.author_pronouns,
                x.edited_at,
            )
        })
    } else {
        let resp = sqlx::query!(
            r#"
SELECT m.*,
       a.name AS author_name,
       a.flags AS author_flags,
       a.discriminator AS author_discriminator,
       a.pronouns AS author_pronouns
FROM messages m
    CROSS JOIN LATERAL (
        SELECT *
        FROM users 
        WHERE id = m.author_id
        ) as a
WHERE channel_id = $1
ORDER BY id DESC
LIMIT $2 OFFSET $3
"#,
            bigint_channel_id,
            limit,
            offset,
        )
        .fetch_all(db)
        .await;

        resp.iter_mut().map(|x| {
            (
                x.id,
                std::mem::take(&mut x.content),
                x.channel_id,
                x.author_id,
                std::mem::take(&mut x.author_name),
                x.author_flags,
                x.author_discriminator,
                x.author_pronouns,
                x.edited_at,
            )
        })
    };

    let output_messages = Vec::with_capacity(messages.len());
    for (
        id,
        content,
        channel_id,
        author_id,
        author_name,
        author_flags,
        author_discriminator,
        author_pronouns,
        edited_at,
    ) in messages
    {
        let author_id = bigdecimal_to_u128!(author_id);
        let id = bigdecimal_to_u128!(id);
        let channel_id = bigdecimal_to_u128!(channel_id);

        Message {
            id,
            content,
            channel_id,
            author_id,
            author: Some(User {
                id: author_id,
                name: author_name,
                avatar: None,
                guilds: None,
                flags: UserFlags::from_bits_truncate(author_flags),
                discriminator: author_discriminator,
                pronouns: author_pronouns.and_then(ferrischat_common::types::Pronouns::from_i16),
            }),
            edited_at,
            embeds: vec![],
            nonce: None,
        }
    }

    Ok(crate::Json {
        obj: MessageHistory { messages },
        code: 200,
    })
}
