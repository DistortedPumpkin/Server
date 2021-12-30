use axum::extract::{Path, Json};
use ferrischat_macros::get_db_or_fail;
use ferrischat_common::types::{ModelType, DMChannel, User, UserFlags};
use ferrischat_common::request_json::{DMChannelCreateJson, CreateDmChannelParams};
use sqlx::types::BigDecimal;


/// POST `/v0/users/me/channels`
pub async fn create_dm_channel(
    auth: crate::Authorization,
    Json(DMChannelCreateJson { group, name }): Json<DMChannelCreateJson>,
    Query(CreateDmChannelParams { users }): Query<CreateDmChannelParams>,
) -> Result<Json<DMChannel>, WebServerError> {
    let db = get_db_or_fail!();

    let node_id = get_node_id!();
    let channel_id = generate_snowflake::<0>(ModelType::DMChannel as u8, node_id);
    let bigint_channel_id = u128_to_bigdecimal!(channel_id);

    let bigint_authed_user = u128_to_bigdecimal!(auth.0);

    let users = users::split(',').filter_map(str::parse::<BigDecimal>).collect();
    
    if !group && users.len() > 1 {
        return Err(ErrorJson::new_400("Direct DM messages can not contain more than 1 other user. \
        Consider making this a group DM to include more people".to_string()).into());
    }

    users.push(bigint_authed_user);

    let res_users = sqlx::query!(
        r#"
        INSERT INTO dmchannels VALUES ($1, $2, $3);

        FOR uid IN $4 LOOP
            INSERT INTO dmmembers VALUES (uid, $1);
        END LOOP;

        SELECT * FROM users WHERE id = ANY($4)
        "#,
        bigint_channel_id,
        group,
        name,
        users
    )
    .fetch_all(db)
    .await?;

    let mut users = Vec::with_capacity(res_users.len());
    for u in res_users {
        users.push(User {
            id: u.id,
            name: u.name.clone(),
            avatar: u.avatar,
            guilds: None,
            flags: UserFlags::from_bits_truncate(u.flags),
            discriminator: u.discriminator,
            pronouns: u
                .pronouns
                .and_then(ferrischat_common::types::Pronouns::from_i16),
        });
    }
    
    let dm_channel_obj = DMChannel {
        id: bigint_channel_id,
        name,
        users,
        group,
    };

    Ok(crate::Json {
        obj: dm_channel_obj,
        code: 201,
    })
}
