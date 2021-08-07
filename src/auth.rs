use rocket::request::{FromRequest, Outcome, Request};

pub struct Credentials {
    pub username: String,
    pub password: String,
}

pub struct BasicAuth {
    pub credentials: Option<Credentials>,
}

fn decode(header: &str) -> Option<Credentials> {
    let decoded = base64::decode(header).ok()?;
    let string = std::str::from_utf8(&decoded).ok()?;
    let mut split = string.split(":");
    let username = split.next()?.to_string();
    let password = split.next()?.to_string();
    Some(Credentials { username, password })
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for BasicAuth {
    type Error = ();

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        match request.headers().get_one("Authorization") {
            Some(value) => {
                if let Some(auth) = decode(value) {
                    Outcome::Success(BasicAuth {
                        credentials: Some(auth),
                    })
                } else {
                    Outcome::Success(BasicAuth { credentials: None })
                }
            }
            None => Outcome::Success(BasicAuth { credentials: None }),
        }
    }
}
