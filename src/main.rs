use btleplug::api::{Central, CentralEvent, Manager as _, ScanFilter, ValueNotification};
use btleplug::platform as btle_plat;
use crossfire::mpmc;
use std::{error, fmt, io, panic, pin, task};
use tokio::{select, sync::oneshot};
use tokio_stream::{Stream, StreamExt, StreamMap};

type AppStateT = ();
type SinkT = ();

#[derive(Debug)]
pub enum AppError {
    BtleError(btleplug::Error),
    Crossfire(mpmc::SendError<AppStateT>),
    GuiRecv(mpmc::TryRecvError),
    Shutdown(ShutdownError),

    IoError(std::io::Error),

    // This should probably be cfg'd out.
    // Use this to throw a random error and test out error paths in the code.
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
            AE::Crossfire(e) => dump!(e),
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
            AE::Crossfire(e) => e.source(),
            AE::GuiRecv(e) => e.source(),
            AE::IoError(e) => e.source(),
            AE::Shutdown(e) => e.source(),
            AE::Sapper => None,
        }
    }
}

fn idle_loop(
    rx: mpmc::RxBlocking<Dev2Gui, mpmc::SharedSenderFRecvB>,
    _: SinkT,
    tx: mpmc::TxBlocking<AppStateT, mpmc::SharedSenderBRecvF>,
) -> Result<(), AppError> {
    let mut finished = false;
    while !finished {
        let rx_msg = rx.try_recv();
        match rx_msg {
            Ok(Dev2Gui::Shutdown) => finished = true,
            Err(mpmc::TryRecvError::Empty) => {}
            Err(e) => return Err(AppError::GuiRecv(e)),
        };
        if !finished {
            let tx_msg = tx.send(());
            match tx_msg {
                // Likely sending too fast, we will try again the next time we receive our idle callback.
                Err(mpmc::SendError(_)) => {}
                Ok(()) => {}
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    println!("Exiting gui thread");
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "needs-consoling")]
    console_subscriber::init();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let sink = ();

    // If we hit this limit we are drop events from the btle thread,
    // so pick something high enough that if we hit it,
    // things have gone awry and the gui probably isn't reading anymore.
    //
    // Or perhaps the btle thread has gone awry and is sending excessively.
    let (tx_dev, rx_gui) = mpmc::bounded_tx_future_rx_blocking(1024);
    // The gui -> Dev bounds don't matter much we just retry next idle loop.
    let (tx_gui, rx_dev) = mpmc::bounded_tx_blocking_rx_future(255);
    let gui_thread_hndl = std::thread::spawn(move || idle_loop(rx_gui, sink, tx_gui));
    let btle_thread_hndl = std::thread::spawn(move || {
        bluetooth_loop(shutdown_rx, tx_dev, rx_dev)

        //let result: Result<(), AppError> = Ok(());
        //let result: Result<(), AppError> = Err(AppError::Sapper);
        //result
    });

    std::thread::sleep(std::time::Duration::from_secs(15));
    println!("shutting down");
    if !shutdown_tx.is_closed() {
        shutdown_tx
            .send(())
            .expect("Unable to shutdown runtime thread");
    } else {
        // This shouldn't normally happen
        eprintln!("shutdown channel was closed before shutdown");
    }

    match gui_thread_hndl.join() {
        Err(e) => panic::resume_unwind(e),
        Ok(result) => result?,
    }

    match btle_thread_hndl.join() {
        Err(e) => panic::resume_unwind(e),
        Ok(result) => result?,
    }

    Ok(())
}

pub enum Dev2Gui {
    Shutdown,
}

fn bluetooth_loop(
    mut shutdown: oneshot::Receiver<()>,
    tx: mpmc::TxFuture<Dev2Gui, mpmc::SharedSenderFRecvB>,
    rx: mpmc::RxFuture<AppStateT, mpmc::SharedSenderBRecvF>,
) -> Result<(), AppError> {
    use tokio::runtime::Builder;
    let rt = Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()?;
    rt.block_on(async {
        let manager = btle_plat::Manager::new().await?;
        let adapters = manager.adapters().await?;
        let adapter = &adapters[0];
        adapter.start_scan(ScanFilter::default()).await?;
        let mut stream_map = StreamMap::new();
        let btle_events = adapter.events().await?;
        stream_map.insert(StreamKey::BtleEvents, EventStreams::BtleEvents(btle_events));
        stream_map.insert(
            StreamKey::GuiState,
            EventStreams::GuiState(rx.into_stream()),
        );
        //events.insert(StreamKey::Notifications, StreamsFoo::Notif(btle_notif));
        /*
        while let Some(event) = stream_map.next().await {
            println!("{:?}", event)
        }
        */
        loop {
            select! {
                event = stream_map.next() => {
                    match &event {
                        Some((StreamKey::BtleEvents, _event)) => {},
                        Some((StreamKey::BtleNotifications(_id), _value)) => {},
                        Some((StreamKey::GuiState, _state)) => {},
                        None => todo!(),
                    }
                    println!("{:?}", event)
                }
                _ = (&mut shutdown) => {
                    let gui = stream_map.remove(&StreamKey::GuiState);
                    let result = match gui {
                        Some(EventStreams::GuiState(_stream)) => {
                            match tx.send(Dev2Gui::Shutdown).await {
                                Err(e) => {
                                    Err(AppError::Shutdown(ShutdownError::MsgSend(e)))
                                }
                                Ok(()) => Ok(()),
                            }
                        }
                        _ => { Err(AppError::Shutdown(ShutdownError::Other)) }
                    };
                    break result;
                }
            }
        }
    })
}

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
enum StreamKey {
    BtleEvents,
    GuiState,
    BtleNotifications(btle_plat::PeripheralId),
}

#[derive(Debug)]
enum EventVariants {
    Event(CentralEvent),
    Notif(ValueNotification),
    GuiState(AppStateT),
}

enum EventStreams {
    BtleEvents(pin::Pin<Box<dyn Stream<Item = CentralEvent> + Send>>),
    BtleNotifications(pin::Pin<Box<dyn Stream<Item = ValueNotification> + Send>>),
    GuiState(
        crossfire::channel::Stream<AppStateT, mpmc::RxFuture<AppStateT, mpmc::SharedSenderBRecvF>>,
    ),
}

impl Unpin for EventStreams {}

impl Stream for EventStreams {
    type Item = EventVariants;

    fn poll_next(
        mut self: pin::Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Option<Self::Item>> {
        let mut this = self.as_mut();

        match &mut *this {
            Self::GuiState(stream) => pin::Pin::new(stream)
                .poll_next(cx)
                .map(|x| x.map(Self::Item::GuiState)),
            Self::BtleEvents(stream) => stream
                .as_mut()
                .poll_next(cx)
                .map(|x| x.map(Self::Item::Event)),
            Self::BtleNotifications(stream) => stream
                .as_mut()
                .poll_next(cx)
                .map(|x| x.map(Self::Item::Notif)),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            Self::BtleEvents(stream) => stream.size_hint(),
            Self::BtleNotifications(stream) => stream.size_hint(),
            Self::GuiState(stream) => stream.size_hint(),
        }
    }
}
