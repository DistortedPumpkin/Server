use axum::extract::{Path, Json};
use ferrischat_macros::get_db_or_fail;
use ferrischat_common::types::{ModelType, DMChannel};
use ferrischat_common::request_json::DMChannelCreateJson;


/// POST `/v0/users/me/channels`
pub async fn create_dm_channel(
    auth: crate::Authorization,
    Json(DMChannelCreateJson { group, name }): Json<DMChannelCreateJson>,
    Path(users): Path<String>,
) -> Result<Json<DMChannel>, WebServerError> {
    let db = get_db_or_fail!();

    let node_id = get_node_id!();
    let channel_id = generate_snowflake::<0>(ModelType::DMChannel as u8, node_id);
    let bigint_channel_id = u128_to_bigdecimal!(channel_id);

    let users = users::split(',').filter_map(str::parse::<u128>).collect();
    
    if !group && users.len() > 1 {
        return Err(ErrorJson::new_400("Direct DM messages can not contain more than 1 other user. \
         Consider making this a group DM to include more people".to_string()).into());
    }

    users.push(auth.0);

    sqlx::query!(
        "INSERT INTO dmchannels VALUES ($1, $2, $3, $4)",
        bigint_channel_id,
        users,
        group,
        name,
    )
    .execute(db)
    .await?;

    let dm_channel_obj = DMChannel {
        id: bigint_channel_id,
        name,
        users,
        group
    };

    Ok(crate::Json {
        obj: dm_channel_obj,
        code: 201,
    })
}