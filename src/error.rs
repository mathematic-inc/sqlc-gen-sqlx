/// An error of the plugin.
#[derive(Debug)]
pub enum Error {
    /// Reading stdin or writing stdout failed.
    Io(std::io::Error),
    /// The request is not valid protobuf.
    Decode(buffa::DecodeError),
    /// The plugin options are not valid JSON.
    Json(serde_json::Error),
    /// The code generation failed. The text says why.
    Codegen(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "io error: {e}"),
            Self::Decode(e) => write!(f, "proto decode error: {e}"),
            Self::Json(e) => write!(f, "json error: {e}"),
            Self::Codegen(msg) => write!(f, "codegen error: {msg}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Decode(e) => Some(e),
            Self::Json(e) => Some(e),
            Self::Codegen(_) => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<buffa::DecodeError> for Error {
    fn from(e: buffa::DecodeError) -> Self {
        Self::Decode(e)
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_io_error() {
        let e = Error::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "file missing",
        ));
        assert!(e.to_string().contains("file missing"));
    }

    #[test]
    fn display_codegen_error() {
        let e = Error::Codegen("unknown type: foo".to_string());
        assert!(e.to_string().contains("unknown type: foo"));
    }

    #[test]
    fn from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::BrokenPipe, "pipe broken");
        let e: Error = io_err.into();
        assert!(matches!(e, Error::Io(_)));
    }

    #[test]
    fn from_json() {
        let json_err = serde_json::from_str::<serde_json::Value>("bad json").unwrap_err();
        let e: Error = json_err.into();
        assert!(matches!(e, Error::Json(_)));
    }
}
