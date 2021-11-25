use crate::ws::{fire_event, WsEventError};

use ferrischat_common::ws::WsOutboundEvent;

use actix_web::web::Json;

use actix_web::{HttpRequest, HttpResponse, Responder};
use ferrischat_common::request_json::MessageUpdateJson;
use ferrischat_common::types::{
    BadRequestJson, InternalServerErrorJson, Message, NotFoundJson, User, UserFlags,
};

pub async fn edit_message(
    req: HttpRequest,
    message_info: Json<MessageUpdateJson>,
    auth: crate::Authorization,
) -> impl Responder {
    let channel_id = get_item_id!(req, "channel_id");
    let message_id = get_item_id!(req, "message_id");

    let bigint_channel_id = u128_to_bigdecimal!(channel_id);
    let bigint_message_id = u128_to_bigdecimal!(message_id);

    let db = get_db_or_fail!();

    let MessageUpdateJson { content } = message_info.0;
    if let Some(ref content) = content {
        if content.len() > 10240 {
            return HttpResponse::BadRequest().json(BadRequestJson {
                reason: "message content size must be fewer than 10,240 bytes".to_string(),
                location: None,
            });
        }
    }

    let guild_id = {
        let resp = sqlx::query!(
            "SELECT guild_id FROM channels WHERE id = $1",
            bigint_channel_id
        )
        .fetch_one(db)
        .await;

        match resp {
            Ok(r) => bigdecimal_to_u128!(r.guild_id),
            Err(e) => {
                return HttpResponse::InternalServerError().json(InternalServerErrorJson {
                    reason: format!("DB returned a error: {}", e),
                    is_bug: false,
                    link: None,
                })
            }
        }
    };

    let old_message = sqlx::query!(
        "SELECT m.*, a.name AS author_name, a.flags AS author_flags, a.discriminator AS author_discriminator, a.pronouns AS author_pronouns FROM messages m CROSS JOIN LATERAL (SELECT * FROM users WHERE id = m.author_id) AS a WHERE m.id = $1 AND m.channel_id = $2",
        bigint_channel_id,
        bigint_message_id
    )
    .fetch_optional(db)
    .await;

    let old_message_obj = match old_message {
        Ok(resp) => match resp {
            Some(resp) => {
                let author_id = bigdecimal_to_u128!(resp.author_id);
                if author_id != auth.0 {
                    return HttpResponse::Forbidden().finish();
                }

                let author_id = bigdecimal_to_u128!(resp.author_id);

                Message {
                    id: message_id,
                    channel_id,
                    author_id,
                    content: resp.content,
                    edited_at: resp.edited_at,
                    embeds: vec![],
                    author: Some(User {
                        id: author_id,
                        name: resp.author_name,
                        avatar: None,
                        guilds: None,
                        flags: UserFlags::from_bits_truncate(resp.author_flags),
                        discriminator: resp.author_discriminator,
                        pronouns: resp
                            .author_pronouns
                            .and_then(ferrischat_common::types::Pronouns::from_i16),
                    }),
                    nonce: None,
                }
            }
            None => {
                return HttpResponse::NotFound().json(NotFoundJson {
                    message: format!("Unknown message with id {}", message_id),
                })
            }
        },
        Err(err) => {
            return HttpResponse::InternalServerError().json(InternalServerErrorJson {
                reason: format!("DB returned an error: {}", err),
                is_bug: false,
                link: None,
            })
        }
    };

    let resp = sqlx::query!("UPDATE messages SET content = $1, edited_at = now()::timestamp without time zone WHERE channel_id = $2 AND id = $3 RETURNING *", content, bigint_channel_id, bigint_message_id)
        .fetch_optional(db)
        .await;

    let message = match resp {
        Ok(resp) => match resp {
            Some(message) => message,
            None => {
                return HttpResponse::NotFound().json(NotFoundJson {
                    message: format!("Unknown message with id {}", message_id),
                })
            }
        },
        Err(e) => {
            return HttpResponse::InternalServerError().json(InternalServerErrorJson {
                reason: format!("DB returned an error: {}", e),
                is_bug: false,
                link: None,
            })
        }
    };

    let new_msg_obj = Message {
        id: message_id,
        channel_id,
        author_id: bigdecimal_to_u128!(message.author_id),
        content: message.content,
        edited_at: message.edited_at,
        embeds: vec![],
        author: old_message_obj.author.clone(),
        nonce: None,
    };

    let event = WsOutboundEvent::MessageUpdate {
        old: old_message_obj,
        new: new_msg_obj.clone(),
    };

    if let Err(e) = fire_event(format!("message_{}_{}", channel_id, guild_id), &event).await {
        let reason = match e {
            WsEventError::MissingRedis => "Redis pool missing".to_string(),
            WsEventError::RedisError(e) => format!("Redis returned an error: {}", e),
            WsEventError::JsonError(e) => {
                format!("Failed to serialize message to JSON format: {}", e)
            }
            WsEventError::PoolError(e) => format!("`deadpool` returned an error: {}", e),
        };
        return HttpResponse::InternalServerError().json(InternalServerErrorJson {
            reason,
            is_bug: true,
            link: Some(
                "https://github.com/FerrisChat/Server/issues/new?assignees=tazz4843&\
                labels=bug&template=api_bug_report.yml&title=%5B500%5D%3A+failed+to+fire+event"
                    .to_string(),
            ),
        });
    }

    HttpResponse::Ok().json(new_msg_obj)
}
