use failure::Fail;

#[derive(Debug, Fail)]
pub enum AuthError {
    #[fail(display = "zone {} already exist", _0)]
    DuplicateZone(String),

    #[fail(display = "zone {} doesn't exist", _0)]
    UnknownZone(String),
}
