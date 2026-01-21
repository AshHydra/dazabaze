use actix_web::{delete, web, HttpRequest, Responder};
use futures_util::TryStreamExt;
use mongodb::bson::doc;

use crate::api::respond;
use crate::auth;
use crate::server::AppState;

/// Delete the current account.
///
/// - Delete organizations owned by the user (and all issues inside them)
/// - Remove the user from member lists of other organizations
/// - Delete the user document
#[delete("/api/me")]
pub async fn me_delete(data: web::Data<AppState>, req: HttpRequest) -> impl Responder {
    let user_id = match auth::require_user_id(&req, &data.jwt_secret) {
        Ok(u) => u,
        Err(e) => return e,
    };

    // Find orgs owned by user
    let mut owned_cursor = match data.organizations.find(doc! { "ownerId": user_id }).await {
        Ok(c) => c,
        Err(_) => return respond::error(actix_web::http::StatusCode::INTERNAL_SERVER_ERROR, "Database error"),
    };

    while let Some(org) = match owned_cursor.try_next().await {
        Ok(v) => v,
        Err(_) => return respond::error(actix_web::http::StatusCode::INTERNAL_SERVER_ERROR, "Database error"),
    } {
        // delete issues in owned org
        if let Err(_) = data.issues.delete_many(doc! { "organizationId": org.id }).await {
            return respond::error(actix_web::http::StatusCode::INTERNAL_SERVER_ERROR, "Database error");
        }
        // delete the org
        if let Err(_) = data.organizations.delete_one(doc! { "_id": org.id }).await {
            return respond::error(actix_web::http::StatusCode::INTERNAL_SERVER_ERROR, "Database error");
        }
    }

    // Remove membership from other orgs
    if let Err(_) = data
        .organizations
        .update_many(doc! {}, doc! { "$pull": { "memberIds": user_id } })
        .await
    {
        return respond::error(actix_web::http::StatusCode::INTERNAL_SERVER_ERROR, "Database error");
    }

    // Delete user
    if let Err(_) = data.users.delete_one(doc! { "_id": user_id }).await {
        return respond::error(actix_web::http::StatusCode::INTERNAL_SERVER_ERROR, "Database error");
    }

    respond::ok_json(serde_json::json!({ "ok": true }))
}

