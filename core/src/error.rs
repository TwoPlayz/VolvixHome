use std::fmt;

/// What went wrong, in words the apps can show as they are.
#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// The server couldn't be reached (wrong address, offline, server down).
    Unreachable(String),
    /// Not signed in, or the sign-in ran out ("Sign in first."). The app goes back to sign-in.
    SignedOut,
    /// The server said no, with its own message ("Name or password is incorrect.").
    Server { status: u16, message: String },
    /// The server answered with something that isn't what a Volvix Home server sends.
    BadAnswer(String),
    /// Reading or writing a file on this device.
    Disk(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Unreachable(why) => write!(f, "Couldn't reach the server ({why})."),
            Error::SignedOut => write!(f, "Sign in first."),
            Error::Server { message, .. } => write!(f, "{message}"),
            Error::BadAnswer(why) => write!(f, "That doesn't look like a Volvix Home server ({why})."),
            Error::Disk(why) => write!(f, "Couldn't save on this computer ({why})."),
        }
    }
}

impl std::error::Error for Error {}

impl From<ureq::Error> for Error {
    fn from(e: ureq::Error) -> Self {
        match e {
            ureq::Error::Status(status, res) => {
                let body = res.into_string().unwrap_or_default();
                let said = serde_json::from_str::<serde_json::Value>(&body)
                    .ok()
                    .and_then(|v| v.get("error").and_then(|m| m.as_str()).map(String::from));
                // a 401 is "sign in first", except a wrong password at sign-in, which keeps its own message
                match (status, said) {
                    (401, None) => Error::SignedOut,
                    (401, Some(m)) if m.starts_with("Sign in") => Error::SignedOut,
                    (status, Some(message)) => Error::Server { status, message },
                    (status, None) => Error::Server { status, message: format!("The server answered {status}.") },
                }
            }
            ureq::Error::Transport(t) => Error::Unreachable(t.kind().to_string()),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Disk(e.to_string())
    }
}
