use crate::device;
use crate::mpmc;
use crate::Dev2Gui;
use std::{error, fmt, io};

#[derive(Debug)]
pub enum AppError {
    BtleError(btleplug::Error),
    Shutdown(FatalError),
    IoError(std::io::Error),
    PacketError(device::ConversionToPacketError),
    MissingPeripheral(btleplug::platform::PeripheralId),
    // This should probably be cfg'd out.
    // Use this to throw a random error and test out error paths in the code.
    #[allow(unused)]
    Sapper,
}

#[derive(Debug)]
pub enum FatalError {
    MsgSend(mpmc::SendError<Dev2Gui>),
    DataRecv(mpmc::TryRecvError),
}

impl<'a> fmt::Display for FatalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        macro_rules! dump {
            ($e: ident) => {
                write!(f, "{:#?}", $e)
            };
        }
        match self {
            FatalError::MsgSend(e) => dump!(e),
            FatalError::DataRecv(e) => dump!(e),
        }
    }
}

impl<'a> error::Error for FatalError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            FatalError::MsgSend(e) => e.source(),
            FatalError::DataRecv(e) => e.source(),
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
impl From<device::ConversionToPacketError> for AppError {
    fn from(e: device::ConversionToPacketError) -> Self {
        AppError::PacketError(e)
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
            AE::PacketError(e) => dump!(e),
            AE::BtleError(e) => dump!(e),
            AE::Shutdown(e) => dump!(e),
            AE::IoError(e) => dump!(e),
            AE::MissingPeripheral(id) => write!(
                f,
                "Cannot find Peripheral for the PeripheralId given {:?}",
                id
            ),
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
            AE::PacketError(e) => e.source(),
            AE::BtleError(e) => e.source(),
            AE::Shutdown(e) => e.source(),
            AE::IoError(e) => e.source(),
            AE::MissingPeripheral(_) => None,
            AE::Sapper => None,
        }
    }
}
