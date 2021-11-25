use crate::ws::{fire_event, WsEventError};
use actix_web::web::Json;
use actix_web::{HttpRequest, HttpResponse, Responder};
use ferrischat_common::request_json::MessageCreateJson;
use ferrischat_common::types::{
    BadRequestJson, InternalServerErrorJson, Message, ModelType, User, UserFlags,
};
use ferrischat_common::ws::WsOutboundEvent;
use ferrischat_snowflake_generator::generate_snowflake;

/// POST `/api/v0/channels/{channel_id}/messages`
pub async fn create_message(
    auth: crate::Authorization,
    req: HttpRequest,
    json: Json<MessageCreateJson>,
) -> impl Responder {
    let MessageCreateJson { content, nonce } = json.0;

    if content.len() > 10240 {
        return HttpResponse::BadRequest().json(BadRequestJson {
            reason: "message content size must be fewer than 10,240 bytes".to_string(),
            location: None,
        });
    }

    let channel_id = get_item_id!(req, "channel_id");
    let bigint_channel_id = u128_to_bigdecimal!(channel_id);

    let node_id = get_node_id!();
    let message_id = generate_snowflake::<0>(ModelType::Message as u8, node_id);
    let bigint_message_id = u128_to_bigdecimal!(message_id);

    let author_id = auth.0;
    let bigint_author_id = u128_to_bigdecimal!(author_id);

    let db = get_db_or_fail!();

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

    let resp = sqlx::query!(
        "INSERT INTO messages VALUES ($1, $2, $3, $4)",
        bigint_message_id,
        content,
        bigint_channel_id,
        bigint_author_id
    )
    .execute(db)
    .await;
    if let Err(e) = resp {
        return HttpResponse::InternalServerError().json(InternalServerErrorJson {
            reason: format!("DB returned a error: {}", e),
            is_bug: false,
            link: None,
        });
    }

    let author = {
        let resp = sqlx::query!("SELECT * FROM users WHERE id = $1", bigint_author_id)
            .fetch_one(db)
            .await;

        match resp {
            Ok(r) => User {
                id: bigdecimal_to_u128!(r.id),
                name: r.name,
                avatar: None,
                guilds: None,
                flags: UserFlags::from_bits_truncate(r.flags),
                discriminator: r.discriminator,
                pronouns: r
                    .pronouns
                    .and_then(ferrischat_common::types::Pronouns::from_i16),
            },
            Err(e) => {
                return HttpResponse::InternalServerError().json(InternalServerErrorJson {
                    reason: format!("DB returned a error: {}", e),
                    is_bug: false,
                    link: None,
                })
            }
        }
    };

    let msg_obj = Message {
        id: message_id,
        content: Some(content),
        channel_id,
        author_id,
        author: Some(author),
        edited_at: None,
        embeds: vec![],
        nonce,
    };

    let event = WsOutboundEvent::MessageCreate {
        message: msg_obj.clone(),
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

    HttpResponse::Created().json(msg_obj)
}
