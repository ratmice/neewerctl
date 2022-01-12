use crate::mpmc;
use crate::oneshot;
use crate::Dev2Gui;
use std::{error, fmt, io};

#[derive(Debug)]
pub enum AppError {
    BtleError(btleplug::Error),
    GuiRecv(mpmc::TryRecvError),
    Shutdown(ShutdownError),

    IoError(std::io::Error),

    // This should probably be cfg'd out.
    // Use this to throw a random error and test out error paths in the code.
    #[allow(unused)]
    Sapper,
}
#[derive(Debug)]
pub enum ShutdownError {
    MsgRecv(oneshot::error::RecvError),
    MsgSend(mpmc::SendError<Dev2Gui>),
    Other,
}

impl<'a> fmt::Display for ShutdownError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        macro_rules! dump {
            ($e: ident) => {
                write!(f, "{:#?}", $e)
            };
        }
        match self {
            ShutdownError::MsgRecv(e) => dump!(e),
            ShutdownError::MsgSend(e) => dump!(e),
            ShutdownError::Other => write!(f, "Encountered other error."),
        }
    }
}

impl<'a> error::Error for ShutdownError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            ShutdownError::MsgRecv(e) => e.source(),
            ShutdownError::MsgSend(e) => e.source(),
            ShutdownError::Other => None,
        }
    }
}

impl From<io::Error> for AppError {
    fn from(e: io::Error) -> Self {
        AppError::IoError(e)
    }
}

impl From<btleplug::Error> for AppError {
    fn from(e: btleplug::Error) -> Self {
        AppError::BtleError(e)
    }
}
impl<'a> fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        macro_rules! dump {
            ($e: ident) => {
                write!(f, "{:#?}", $e)
            };
        }
        use AppError as AE;
        match self {
            AE::BtleError(e) => dump!(e),
            AE::GuiRecv(e) => dump!(e),
            AE::Shutdown(e) => dump!(e),
            AE::IoError(e) => dump!(e),

            AE::Sapper => write!(
                f,
                "This error should only occurr during internal testing..."
            ),
        }
    }
}

impl<'a> error::Error for AppError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        use AppError as AE;
        match self {
            AE::BtleError(e) => e.source(),
            AE::GuiRecv(e) => e.source(),
            AE::IoError(e) => e.source(),
            AE::Shutdown(e) => e.source(),
            AE::Sapper => None,
        }
    }
}
