use btleplug::api::{Central, Manager as _, ScanFilter};
use btleplug::platform as btle_plat;
use crossfire::mpmc;
use std::panic;
use tokio::{select, sync::oneshot};
use tokio_stream::{StreamExt, StreamMap};

type AppStateT = ();
type SinkT = ();

mod errors;
mod appstate;
mod streams;

use errors::*;
use appstate::*;
use streams::*;

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

    //
    // If we hit this limit we are drop events from the btle thread,
    // so pick something high enough that if we hit it,
    // things have gone awry and the gui probably isn't reading anymore.
    //
    // Or perhaps the btle thread has gone awry and is sending excessively.
    //
    // One might ask why not use an unbounded stream?
    // Well, I haven't found an unbounded mpmc channel that works with
    // the idle thread, I.e. sync, clone
    // Maybe crossfires should, But it didn't when I tried it last
    // (FIXME retry and remember why it didnt).
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
                msg = (&mut shutdown) => {
                    return match msg {
                        Ok(()) => {
                            let gui = stream_map.remove(&StreamKey::GuiState);
                            match gui {
                                Some(EventStreams::GuiState(_stream)) => {
                                    match tx.send(Dev2Gui::Shutdown).await {
                                        Err(e) => {
                                            Err(AppError::Shutdown(ShutdownError::MsgSend(e)))
                                        }
                                        Ok(()) => Ok(()),
                                    }
                                }
                                _ => { Err(AppError::Shutdown(ShutdownError::Other)) }
                            }
                        }
                        Err(e) => {
                            Err(AppError::Shutdown(ShutdownError::MsgRecv(e)))
                        }
                    }
                }
            }
        }
    })
}
