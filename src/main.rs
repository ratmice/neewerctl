use crossfire::mpmc;
use std::{error, fmt, panic};
use tokio::sync::oneshot;

#[derive(Debug)]
pub enum AppError {
    BtleError(btleplug::Error),
    Crossfire(crossfire::mpmc::SendError<AppStateT>),
    Shutdown(tokio::sync::oneshot::error::RecvError),
    // This should probably be cfg'd out.
    // Use this to throw a random error and test out error paths in the code.
    Sapper,
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
            AE::Crossfire(e) => dump!(e),
            AE::Shutdown(e) => dump!(e),
            AE::Sapper => write!(f, "This error should only occurr during internal testing..."),
        }
    }
}

impl<'a> error::Error for AppError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        use AppError as AE;
        match self {
            AE::BtleError(e) => e.source(),
            AE::Crossfire(e) => e.source(),
            AE::Shutdown(e) => e.source(),
            AE::Sapper => None,

        }}
}

type AppStateT = ();
type SinkT = ();
fn start_gui_idle_thread(
    rx_gui: crossfire::mpmc::RxBlocking<AppStateT, crossfire::mpmc::SharedSenderFRecvB>,
    sink: SinkT,
    tx_gui: crossfire::mpmc::TxBlocking<AppStateT, crossfire::mpmc::SharedSenderBRecvF>,
) {

}


fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "needs-consoling")]
    console_subscriber::init();
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let sink = ();

    // Lets go with a small bounds, that is slightly larger than we imagine occurring,
    // and see if we hit it!
    let (tx_dev, rx_gui) = crossfire::mpmc::bounded_tx_future_rx_blocking(8);
    let (tx_gui, rx_dev) = crossfire::mpmc::bounded_tx_blocking_rx_future(8);
    let _gui_thread_hndl = start_gui_idle_thread(rx_gui, sink, tx_gui);
    let btle_thread_hndl = std::thread::spawn(move || {
        bluetooth_loop(shutdown_rx, tx_dev, rx_dev)

        //let result: Result<(), AppError> = Ok(());
        //let result: Result<(), AppError> = Err(AppError::Testing);
        //result
    });

    println!("shutting down");
    if !shutdown_tx.is_closed() {
        shutdown_tx
            .send(())
            .expect("Unable to shutdown runtime thread");
    } else {
        // This shouldn't normally happen
        eprintln!("shutdown channel was closed before shutdown");
    }

    match btle_thread_hndl.join() {
        Err(e) => panic::resume_unwind(e),
        Ok(result) => result?,
    }

    Ok(())
}

fn bluetooth_loop(
    mut shutdown: oneshot::Receiver<()>,
    tx: mpmc::TxFuture<AppStateT, mpmc::SharedSenderFRecvB>,
    rx: mpmc::RxFuture<AppStateT, mpmc::SharedSenderBRecvF>,
) -> Result<(), AppError> {
    Ok(())
}
