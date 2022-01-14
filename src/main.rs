use btleplug::api::{Central, Manager as _, Peripheral as _, ScanFilter};
use btleplug::platform as btle_plat;
use crossfire::mpmc;
use druid::{
    commands, AppLauncher, /* Data, */ Env, LocalizedString, Menu, MenuItem, SysMods,
    WindowDesc, WindowId,
};
use imbl as im;
//use imbl::HashMap;
//use btle_plat::PeripheralId;
//use std::borrow::BorrowMut;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::{panic, sync::Arc};
use tokio::{select, sync::oneshot};
use tokio_stream::{StreamExt, StreamMap};

type AppStateT = appstate::AppState;
type SinkT = druid::ExtEventSink;

mod appstate;
mod device;
mod errors;
mod streams;
mod widgets;

use appstate::*;
use device::*;
use errors::*;
use streams::*;
use widgets::*;
static QUIT: AtomicBool = AtomicBool::new(false);

fn idle_loop(
    rx: mpmc::RxBlocking<Dev2Gui, mpmc::SharedSenderFRecvB>,
    sink: SinkT,
    tx: mpmc::TxBlocking<Gui2Dev, mpmc::SharedSenderBRecvF>,
) -> Result<(), AppError> {
    while !QUIT.load(Relaxed) {
        let tx = tx.clone();
        let rx_msg = rx.try_recv();
        let _local_state = AppState::default();

        // This needs to be done outside of the idle callback.
        //
        // If the gui exits and a shutdown is then sent,
        // The idle callback may never be called.
        if let Ok(Dev2Gui::Shutdown) = rx_msg {
            QUIT.store(true, Relaxed);
            continue;
        }

        if !QUIT.load(Relaxed) {
            sink.add_idle_callback(move |app_state: &mut AppStateT| {
                let all_lights = Arc::make_mut(&mut app_state.lights);
                let incoming = match &rx_msg {
                    Ok(Dev2Gui::Connected(incoming)) => Some(incoming),
                    Ok(Dev2Gui::Disconnected(incoming)) => Some(incoming),
                    Ok(Dev2Gui::Changed(incoming)) => Some(incoming),
                    Err(mpmc::TryRecvError::Empty) => None,
                    Err(_e) => {
                        // It'd be better to propagate this to the gui somehow,
                        // probably fall-through... After that, we'll need to set finish.
                        //
                        QUIT.store(true, Relaxed);
                        return;
                    }
                    Ok(Dev2Gui::Shutdown) => {
                        unreachable!()
                    }
                };
                let mut outgoing = im::HashMap::new();
                for (id, light) in all_lights.iter_mut() {
                    if let Some(incoming) = incoming {
                        if let Some(dev_light) = incoming.get(id) {
                            if light.has_changes() {
                                outgoing.insert(id.clone(), light.clone());
                            }
                            light.sync(dev_light)
                        }
                    }
                }
                let tx_msg = tx.send(Gui2Dev::Changed(outgoing));
                match tx_msg {
                    // Likely sending too fast, we will try again the next time we receive our idle callback.
                    Err(mpmc::SendError(_)) => {}
                    Ok(()) => {}
                }

                // update the app state with the rx_msg.
                let _state = Arc::make_mut(&mut app_state.lights);
            });
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    println!("Exiting gui thread");
    Ok(())
}

fn main_menu(_id: Option<WindowId>, _data: &AppState, _env: &Env) -> Menu<AppState> {
    Menu::new(LocalizedString::new("gtk-menu-application-menu")).entry(
        MenuItem::new(LocalizedString::new("gtk-menu-quit-app"))
            // druid handles the QUIT_APP command automatically
            .command(commands::QUIT_APP)
            .hotkey(SysMods::Cmd, "q"),
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "needs-consoling")]
    console_subscriber::init();

    let window = WindowDesc::new(devices_widget())
        .title(LocalizedString::new("neewerctl-app").with_placeholder("neewerctl"))
        .menu(main_menu);
    let launcher = AppLauncher::with_window(window);

    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let sink = launcher.get_external_handle();

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

    launcher.launch(AppState::default()).expect("launch failed");

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

#[derive(Debug)]
pub enum Dev2Gui {
    Connected(im::HashMap<btle_plat::PeripheralId, Light>),
    Disconnected(im::HashMap<btle_plat::PeripheralId, Light>),
    Changed(im::HashMap<btle_plat::PeripheralId, Light>),
    Shutdown,
}
#[derive(Debug)]
pub enum Gui2Dev {
    Changed(im::HashMap<btle_plat::PeripheralId, Light>),
}

fn bluetooth_loop(
    mut shutdown: oneshot::Receiver<()>,
    tx: mpmc::TxFuture<Dev2Gui, mpmc::SharedSenderFRecvB>,
    rx: mpmc::RxFuture<Gui2Dev, mpmc::SharedSenderBRecvF>,
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
        let mut peripherals: im::HashMap<btle_plat::PeripheralId, btle_plat::Peripheral> = im::HashMap::new();
        let mut disconnected_ids: im::HashSet<btle_plat::PeripheralId> = im::HashSet::new();
        loop {
            select! {
                event = stream_map.next() => {
                    match &event {
                        Some((StreamKey::BtleEvents, EventVariants::Event(event))) => {
                            match event {
                                btleplug::api::CentralEvent::DeviceDiscovered(_id) => {},
                                btleplug::api::CentralEvent::DeviceUpdated(_id) => {},
                                btleplug::api::CentralEvent::DeviceConnected(id) => {
                                    let peripheral = adapter.peripheral(id).await.unwrap();
                                    let notifs = peripheral.notifications().await.unwrap();
                                    peripherals.insert(id.clone(), peripheral.clone());
                                    stream_map.insert(StreamKey::BtleNotifications(id.clone()), EventStreams::BtleNotifications(notifs));
                                },
                                btleplug::api::CentralEvent::DeviceDisconnected(id) => {
                                    disconnected_ids.insert(id.clone());
                                    peripherals.remove(id);
                                    stream_map.remove(&StreamKey::BtleNotifications(id.clone()));
                                },
                                btleplug::api::CentralEvent::ManufacturerDataAdvertisement { .. } => {},
                                btleplug::api::CentralEvent::ServiceDataAdvertisement { .. } => {},
                                btleplug::api::CentralEvent::ServicesAdvertisement { .. } => {},
                            }
                        },
                        Some((StreamKey::BtleNotifications(_id), EventVariants::Notification(_value))) => {},
                        Some((StreamKey::GuiState, EventVariants::GuiState(_state))) => {},
                        Some((_, _)) => unreachable!(),
                        None => todo!(),
                    }
                    println!("{:?}", event)
                }
                shutdown_msg = (&mut shutdown) => {
                    println!("Shutting down: {:?}", shutdown_msg);
                    return match shutdown_msg {
                        Ok(()) => {
                            let gui = stream_map.remove(&StreamKey::GuiState);
                            match gui {
                                Some(EventStreams::GuiState(_stream)) => {
                                    match tx.send(Dev2Gui::Shutdown).await {
                                        Err(e) => {
                                            Err(AppError::Shutdown(ShutdownError::MsgSend(e)))
                                        }
                                        Ok(()) => { Ok(()) },
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
