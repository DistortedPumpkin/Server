use crate::auth::token_gen::generate_random_bits;
use actix_web::web::HttpResponse;
use actix_web::{HttpRequest, Responder};
use ferrischat_common::types::{BadRequestJson, BadRequestJsonLocation, InternalServerErrorJson};
use ferrischat_macros::{get_db_or_fail, get_item_id};
use num_traits::FromPrimitive;
use sqlx::types::BigDecimal;
use tokio::sync::oneshot::channel;

pub async fn get_token(req: HttpRequest) -> impl Responder {
    let token = match generate_random_bits() {
        Some(b) => base64::encode_config(b, base64::URL_SAFE),
        None => {
            return HttpResponse::InternalServerError().json(InternalServerErrorJson {
                reason: "failed to generate random bits for token generation".to_string(),
            })
        }
    };
    let user_id = get_item_id!(req, "user_id");
    let headers = req.headers();
    let user_email = match headers.get("Email") {
        Some(e) => match String::from_utf8(Vec::from(e.as_bytes())) {
            Ok(e) => e,
            Err(e) => {
                return HttpResponse::BadRequest().json(BadRequestJson {
                    reason: "`Email` header contained invalid UTF-8".to_string(),
                    location: Some(BadRequestJsonLocation {
                        line: 0,
                        character: (e.utf8_error().valid_up_to() + 1) as u32,
                    }),
                })
            }
        },
        None => {
            return HttpResponse::BadRequest().json(BadRequestJson {
                reason: "No `Email` header passed".to_string(),
                location: None,
            })
        }
    };
    let user_password = match headers.get("Password") {
        Some(p) => match String::from_utf8(Vec::from(p.as_bytes())) {
            Ok(p) => p,
            Err(e) => {
                return HttpResponse::BadRequest().json(BadRequestJson {
                    reason: "`Email` header contained invalid UTF-8".to_string(),
                    location: Some(BadRequestJsonLocation {
                        line: 0,
                        character: (e.utf8_error().valid_up_to() + 1) as u32,
                    }),
                })
            }
        },
        None => {
            return HttpResponse::BadRequest().json(BadRequestJson {
                reason: "No `Password` header passed".to_string(),
                location: None,
            })
        }
    };

    let db = get_db_or_fail!();

    let bigint_user_id = BigDecimal::from_u128(user_id);
    match sqlx::query!(
        "SELECT email, password FROM users WHERE id = $1",
        bigint_user_id
    )
    .fetch_one(db)
    .await
    {
        Ok(r) => {
            if !((user_email == r.email) && (user_password == r.password)) {
                return HttpResponse::Unauthorized().finish();
            }
        }
        Err(e) => {
            return HttpResponse::InternalServerError().json(InternalServerErrorJson {
                reason: format!("DB returned a error: {}", e),
            })
        }
    };

    let hashed_token = {
        let rx = match crate::GLOBAL_HASHER.get() {
            Some(h) => {
                let (tx, rx) = channel();
                if h.send((token.clone(), tx)).await.is_err() {
                    return HttpResponse::InternalServerError().json(InternalServerErrorJson {
                        reason: "Password hasher has hung up connection".to_string(),
                    });
                };
                rx
            }
            None => {
                return HttpResponse::InternalServerError().json(InternalServerErrorJson {
                    reason: "password hasher not found".to_string(),
                })
            }
        };
        match rx.await {
            Ok(r) => match r {
                Ok(r) => r,
                Err(e) => {
                    return HttpResponse::InternalServerError().json(InternalServerErrorJson {
                        reason: format!("failed to hash token: {}", e),
                    })
                }
            },
            Err(e) => unreachable!(
                "failed to receive value from channel despite value being sent earlier on: {}",
                e
            ),
        }
    };

    if let Err(e) = sqlx::query!("INSERT INTO auth_tokens VALUES ($1, $2) ON CONFLICT (user_id) DO UPDATE SET auth_token = $2", BigDecimal::from_u128(user_id), hashed_token).execute(db).await {
        return HttpResponse::InternalServerError().json(InternalServerErrorJson {
            reason: format!("DB returned a error: {}", e)
        })
    };

    return HttpResponse::Ok().body(format!(
        "{}.{}",
        base64::encode_config(user_id.to_string(), base64::URL_SAFE),
        token,
    ));
}
