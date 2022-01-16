use btle_plat::PeripheralId;
use btleplug::api::{
    Central, Manager as _, Peripheral as _, ScanFilter, ValueNotification, WriteType,
};
use btleplug::platform as btle_plat;
use crossfire::mpmc;
use druid::{
    commands, AppLauncher, Env, LocalizedString, Menu, MenuItem, SysMods, WindowDesc, WindowId,
};
use im::hashmap::Entry as ImEntry;
use imbl as im;
use std::{convert::TryFrom, panic, sync::Arc};
use tokio::{select, sync::oneshot};
use tokio_stream::{StreamExt, StreamMap};
use tracing_log::log;

type AppStateT = appstate::AppState;
type SinkT = druid::ExtEventSink;

mod appstate;
mod device;
mod errors;
mod streams;
mod widgets;
mod semaphore;

use appstate::*;
use errors::*;
use streams::*;
use widgets::*;
use semaphore::*;

use device::Packet as _;

fn idle_loop(
    rx: mpmc::RxBlocking<Dev2Gui, mpmc::SharedSenderFRecvB>,
    sink: SinkT,
    tx: mpmc::TxBlocking<Gui2Dev, mpmc::SharedSenderBRecvF>,
) -> Result<(), AppError> {
    loop {
        let tx = tx.clone();
        let mut received = im::HashMap::new();
        loop {
            match rx.try_recv() {
                Ok(Dev2Gui::Shutdown) => {
                    return Ok(());
                }
                Err(mpmc::TryRecvError::Empty) => {
                    break;
                }
                Ok(Dev2Gui::DeviceEvent(dev_event)) => {
                    received.insert(dev_event.id(), dev_event);
                }
                Err(tryrecv) => {
                    return Err(AppError::Shutdown(FatalError::DataRecv(tryrecv)));
                }
            };
        }

        sink.add_idle_callback(move |app_state: &mut AppStateT| {
            let gui_lights = Arc::make_mut(&mut app_state.lights);
            for (gui_id, gui_light) in &mut gui_lights.iter_mut() {
                if let ImEntry::Occupied(occupied) = received.entry(gui_id.clone()) {
                    match occupied.get() {
                        DeviceEvent::Connected(_, _) => {
                            gui_light.connected = true;
                        }
                        DeviceEvent::Disconnected(_, _) => {
                            gui_light.connected = false;
                        }
                        DeviceEvent::Changed(_, dev_light) => {
                            gui_light.sync(dev_light);
                        }
                    }
                    occupied.remove();
                }
                if gui_light.has_changes() {
                    if let Ok(()) = tx.send(Gui2Dev::Changed(gui_id.clone(), gui_light.clone())) {
                        gui_light.clear_changes()
                    }
                }
            }

            for (dev_id, dev_event) in received {
                gui_lights.insert(dev_id, dev_event.light());
            }
        });
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
}

fn main_menu(_id: Option<WindowId>, _data: &AppState, _env: &Env) -> Menu<AppState> {
    Menu::empty().entry(
    Menu::new(LocalizedString::new("common-menu-file-menu"))
    .entry(
                MenuItem::new(LocalizedString::new("common-menu-file-close"))
                .command(commands::CLOSE_WINDOW)

    ).entry(
        MenuItem::new(LocalizedString::new("Quit"))
            .command(commands::QUIT_APP)
            .hotkey(SysMods::Cmd, "q"),
    ))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "needs-consoling")]
    console_subscriber::init();
    #[cfg(not(feature = "needs-consoling"))]
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let window = WindowDesc::new(devices_widget())
        .title(LocalizedString::new("neewerctl").with_placeholder("neewerctl"))
        .menu(main_menu);
    let launcher =
        AppLauncher::with_window(window).configure_env(druid_widget_nursery::configure_env);

    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let sink = launcher.get_external_handle();

    let hndl = start_threads(shutdown_rx, sink);
    launcher.launch(AppState::default()).expect("launch failed");

    if !shutdown_tx.is_closed() {
        shutdown_tx
            .send(())
            .expect("Unable to shutdown runtime thread");
    } else {
        // This shouldn't normally happen
        log::error!("shutdown channel was closed before shutdown");
    }

    let _ = hndl.join();

    Ok(())
}

fn start_threads(shutdown_rx: tokio::sync::oneshot::Receiver<()>, sink: druid::ExtEventSink) -> std::thread::JoinHandle<()> {
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
            btle_loop(shutdown_rx, tx_dev, rx_dev)

            //let result: Result<(), AppError> = Ok(());
            //let result: Result<(), AppError> = Err(AppError::Sapper);
        });

        std::thread::spawn(move || {
            match btle_thread_hndl.join() {
                    Err(e) => panic::resume_unwind(e),
                    Ok(Ok(())) => {
                        log::info!("BTLE thread exited ok.")
                    }
                    Ok(Err(e)) => {
                        log::info!("BTLE thread exited with error: {:?}", e)
                    }
            };
            // Order matters here because btle_thread_hndl tells this one to shutdown,
            // If we joined gui_thread_hndl first, everything might exit before
            // gui_thread shutds down.
            match gui_thread_hndl.join() {
                Err(e) => panic::resume_unwind(e),
                Ok(Ok(())) => {
                        log::info!("GUI thread exited ok.")
                }
                Ok(Err(e)) => log::info!("GUI Thread exited with error: {:?}", e),
            };
        })
}

#[derive(Debug, Clone)]
pub enum DeviceEvent {
    Connected(PeripheralId, Light),
    Disconnected(PeripheralId, Light),
    Changed(PeripheralId, Light),
}
pub enum Dev2Gui {
    DeviceEvent(DeviceEvent),
    Shutdown,
}
impl DeviceEvent {
    fn id(&self) -> PeripheralId {
        match self {
            Self::Connected(id, _) | Self::Disconnected(id, _) | Self::Changed(id, _) => id.clone(),
        }
    }
    fn light(&self) -> Light {
        match self {
            Self::Connected(_, l) | Self::Disconnected(_, l) | Self::Changed(_, l) => l.clone(),
        }
    }
}

#[derive(Debug)]
pub enum Gui2Dev {
    Changed(PeripheralId, Light),
}

type AsyncSender = mpmc::TxFuture<Dev2Gui, mpmc::SharedSenderFRecvB>;
type AsyncReceiver = mpmc::RxFuture<Gui2Dev, mpmc::SharedSenderBRecvF>;

fn btle_loop (
    mut shutdown: oneshot::Receiver<()>,
    tx: AsyncSender,
    rx: AsyncReceiver,
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
        let mut peripherals: im::HashMap<PeripheralId, btle_plat::Peripheral> = im::HashMap::new();
        let mut disconnected: im::HashSet<PeripheralId> = im::HashSet::new();
        loop {
            select! {
                event = stream_map.next() => {
                    let result = match &event {
                        Some((StreamKey::BtleEvents, EventVariants::Event(event))) => {
                            handle_btle_event(event, adapter, &mut peripherals, &mut disconnected, &mut stream_map, tx.clone()).await
                        },
                        Some((StreamKey::BtleNotifications(id), EventVariants::Notification(value))) => {
                            handle_btle_notification(id, value, tx.clone()).await
                        },
                        Some((StreamKey::GuiState, EventVariants::GuiState(Gui2Dev::Changed(id,state)))) => {
                            if let Some(peripheral) = peripherals.get(id) {
                                handle_gui_event(peripheral, state).await
                            } else {
                                Err(AppError::MissingPeripheral(id.clone()))
                            }
                        },
                        Some((_, _)) => unreachable!(),
                        None => todo!(),
                    };
                    match result {
                        Ok(id) => {
                            if disconnected.contains(&id) || peripherals.contains_key(&id) {
                                log::info!("{:?}", &event)
                            };
                        }
                        Err(e) => {
                            log::warn!("Error: {:#?}", e);
                        }
                    };
                }
                shutdown_msg = (&mut shutdown) => {
                    return match shutdown_msg {
                        Ok(()) => {
                            let result = match tx.send(Dev2Gui::Shutdown).await {
                                 Err(e) => Err(AppError::Shutdown(FatalError::MsgSend(e))),
                                 Ok(()) => Ok(()),
                            };
                            result
                        }
                        Err(e) => {
                            Err(AppError::Shutdown(FatalError::ShutdownRecv(e)))
                        }
                    }
                }
            }
        }
    })
}

async fn handle_btle_notification(
    id: &PeripheralId,
    _value: &ValueNotification,
    _tx: AsyncSender,
) -> Result<PeripheralId, AppError> {
    Ok(id.clone())
}

async fn handle_gui_event(
    peripheral: &btle_plat::Peripheral,
    state: &appstate::Light,
) -> Result<PeripheralId, AppError> {
    for change in state.changes() {
        match change {
            Changed::Mode => {
                // Should probably use a ref instead of try_from.
                // With that I believe we could return msg.bytes, and
                // have only one call to write...
                match &state.mode {
                    LightMode::CCT(mode) => {
                        let pkt = device::CCT::try_from(mode.clone())?;
                        let msg = pkt.bytes();
                        peripheral
                            .write(&device::DEV_CTL, msg, WriteType::WithoutResponse)
                            .await?;
                    }
                    LightMode::HSI(mode) => {
                        let pkt = device::HSI::try_from(mode.clone())?;
                        let msg = pkt.bytes();
                        peripheral
                            .write(&device::DEV_CTL, msg, WriteType::WithoutResponse)
                            .await?;
                    }
                    LightMode::Anim(mode) => {
                        let pkt = device::Anim::try_from(mode.clone())?;
                        let msg = pkt.bytes();
                        peripheral
                            .write(&device::DEV_CTL, msg, WriteType::WithoutResponse)
                            .await?;
                    }
                }
            }
            Changed::Power => {
                let pkt = device::Power::from(state.power);
                let msg = pkt.bytes();
                peripheral
                    .write(&device::DEV_CTL, msg, WriteType::WithoutResponse)
                    .await?;
                peripheral
                    .write(
                        &device::DEV_CTL,
                        &device::POWER_STATUS,
                        WriteType::WithoutResponse,
                    )
                    .await?;
                peripheral
                    .write(
                        &device::DEV_CTL,
                        &device::CHANNEL_STATUS,
                        WriteType::WithoutResponse,
                    )
                    .await?;
            }
            Changed::Connected => {}
        }
    }
    Ok(peripheral.id())
}

async fn handle_btle_event(
    event: &btleplug::api::CentralEvent,
    adapter: &btle_plat::Adapter,
    peripherals: &mut im::HashMap<PeripheralId, btle_plat::Peripheral>,
    disconnected: &mut im::HashSet<PeripheralId>,
    stream_map: &mut StreamMap<StreamKey, EventStreams>,
    tx: AsyncSender,
) -> Result<PeripheralId, AppError> {
    let result: Result<PeripheralId, AppError> = match event {
        btleplug::api::CentralEvent::DeviceDiscovered(id) => {
            let peripheral: btle_plat::Peripheral = adapter.peripheral(id).await?;
            let properties = peripheral.properties().await?;
            if let Some(properties) = properties {
                if Some("NEEWER-RGB480".to_string()) == properties.local_name {
                    peripheral.connect().await?;
                    peripherals.insert(id.clone(), peripheral.clone());
                }
            };
            Ok(id.clone())
        }
        btleplug::api::CentralEvent::DeviceUpdated(id) => {
            if disconnected.contains(id) {
                let peripheral = adapter.peripheral(id).await?;
                peripheral.connect().await?;
            };
            Ok(id.clone())
        }
        btleplug::api::CentralEvent::DeviceConnected(id) => {
            let peripheral = adapter.peripheral(id).await?;
            peripheral
                .discover_services()
                .await
                .expect("Discovering services");
            let characteristics = peripheral.characteristics();
            assert!(
                characteristics.contains(&device::GATT)
                    && characteristics.contains(&device::DEV_CTL)
            );
            let notifs = peripheral.notifications().await?;
            stream_map.insert(
                StreamKey::BtleNotifications(id.clone()),
                EventStreams::BtleNotifications(notifs),
            );
            peripheral.subscribe(&device::GATT).await?;
            disconnected.remove(id);
            peripherals.insert(id.clone(), peripheral.clone());
            tx.send(Dev2Gui::DeviceEvent(DeviceEvent::Connected(
                id.clone(),
                Light::default(),
            )))
            .await
            .map_err(|e| AppError::Shutdown(FatalError::MsgSend(e)))?;
            peripheral
                .write(
                    &device::DEV_CTL,
                    &device::POWER_STATUS,
                    WriteType::WithoutResponse,
                )
                .await?;
            peripheral
                .write(
                    &device::DEV_CTL,
                    &device::CHANNEL_STATUS,
                    WriteType::WithoutResponse,
                )
                .await?;
            Ok(id.clone())
        }
        btleplug::api::CentralEvent::DeviceDisconnected(id) => {
            disconnected.insert(id.clone());
            peripherals.remove(id);
            stream_map.remove(&StreamKey::BtleNotifications(id.clone()));
            Ok(id.clone())
        }
        btleplug::api::CentralEvent::ManufacturerDataAdvertisement { id, .. } => Ok(id.clone()),
        btleplug::api::CentralEvent::ServiceDataAdvertisement { id, .. } => Ok(id.clone()),
        btleplug::api::CentralEvent::ServicesAdvertisement { id, .. } => Ok(id.clone()),
    };

    result
}
