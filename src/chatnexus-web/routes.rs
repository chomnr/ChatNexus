use oauth2::{AuthorizationCode, reqwest::async_http_client, TokenResponse};
use rocket::futures::lock::Mutex;
use rocket::{Route, routes, get, response::Redirect, State, post, serde::json::Json};
use rocket::http::{CookieJar, Cookie};
use serde::{Serialize, Deserialize};
use serde_json::{json, Value};
use tonic::transport::Channel;

use crate::chatnexus_chat::{AuthRequest, AuthVerifyRequest, AuthStatus};
use crate::oauth2::DiscordUser;
use crate::{oauth2::OAuth2, chatnexus_chat::auth_client::AuthClient};

#[derive(Serialize, Deserialize)]
pub struct IdentityCheck {
    session_id: String,
    code: String
}


#[get("/?<code>")]
async fn index(code: String, oauth2: &State<OAuth2>, jar: &CookieJar<'_>) -> Result<Redirect, Value> {
    let result = oauth2.exchange_auth_code(code).await;
    let data = oauth2.post_discord(result.access_token()).await;
    match data {
        Ok(val) => {
            let cookie = Cookie::build("sid", serde_json::to_string(&val).unwrap())
                .same_site(rocket::http::SameSite::None)
                .secure(false) // important to enable this if you have https..
                .finish();
            jar.add_private(cookie);
            Ok(Redirect::to("/verify"))
        },
        Err(_) => return Err (
            json!({
                "message": "failed to authorize with the intermediator"
            })
        ),
    }
}

#[post("/identity/check", data = "<identity>")]
async fn identity_check(identity: Json<IdentityCheck>, auth: &State<Mutex<AuthClient<Channel>>>, jar: &CookieJar<'_>) -> Value {
    let creds = identity.0;

    if jar.get_private("sid").is_some() {
        let discord_user: DiscordUser = serde_json::from_str(jar.get_private("sid").unwrap().value()).unwrap();
        let request = AuthVerifyRequest {
            session_id: creds.session_id,
            code: creds.code,
            uid: discord_user.id().to_string(),
            secret_key: dotenv::var("WEB_SECRET_KEY").unwrap().to_string(),
            username: discord_user.username().to_string(),
            discriminator: discord_user.discriminator().to_string(),
        };
        let status = auth.lock().await.verify_user(request).await.unwrap();
        return json!({
            "status": status.get_ref().status()
        })
    }
    return json!({
        "status": AuthStatus::Denied
    })
}

pub fn routes() -> Vec<Route> {
    routes![index, identity_check]
}
