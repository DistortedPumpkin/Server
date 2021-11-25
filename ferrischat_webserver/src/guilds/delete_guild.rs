use crate::ws::{fire_event, WsEventError};
use ferrischat_common::ws::WsOutboundEvent;

use actix_web::{HttpRequest, HttpResponse, Responder};
use ferrischat_common::types::{Guild, GuildFlags, InternalServerErrorJson, Member, NotFoundJson};

/// DELETE `/api/v0/guilds/{guild_id}`
pub async fn delete_guild(req: HttpRequest, auth: crate::Authorization) -> impl Responder {
    let db = get_db_or_fail!();
    let guild_id = get_item_id!(req, "guild_id");
    let bigint_guild_id = u128_to_bigdecimal!(guild_id);
    let bigint_user_id = u128_to_bigdecimal!(auth.0);

    let resp = sqlx::query!(
        "DELETE FROM guilds WHERE id = $1 AND owner_id = $2 RETURNING *",
        bigint_guild_id,
        bigint_user_id
    )
    .fetch_optional(db)
    .await;

    let guild_obj = match resp {
        Err(e) => {
            return HttpResponse::InternalServerError().json(InternalServerErrorJson {
                reason: format!("DB returned a error: {}", e),
                is_bug: false,
                link: None,
            })
        }
        Ok(r) => match r {
            Some(r) => {
                if bigdecimal_to_u128!(r.owner_id) == auth.0 {
                    Guild {
                        id: guild_id,
                        owner_id: auth.0,
                        name: r.name,
                        channels: None,
                        flags: GuildFlags::empty(),
                        members: Some(vec![Member {
                            guild_id: Some(guild_id),
                            user_id: Some(auth.0),
                            user: None,
                            guild: None,
                        }]),
                        roles: None,
                    }
                } else {
                    return HttpResponse::Forbidden().finish();
                }
            }
            None => {
                return HttpResponse::NotFound().json(NotFoundJson {
                    message: format!(
                        "Unknown guild with id {0} or owner_id {1}",
                        guild_id, auth.0
                    ),
                })
            }
        },
    };

    let event = WsOutboundEvent::GuildDelete {
        guild: guild_obj.clone(),
    };

    if let Err(e) = fire_event(format!("guild_{}", guild_id), &event).await {
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

    HttpResponse::NoContent().finish()
}
