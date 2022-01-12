use crate::btle_plat;
use crate::mpmc;
use crate::AppStateT;
use btleplug::api::{CentralEvent, ValueNotification};
use std::{pin, task};
use tokio_stream::Stream;

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub enum StreamKey {
    BtleEvents,
    GuiState,
    BtleNotifications(btle_plat::PeripheralId),
}

#[derive(Debug)]
pub enum EventVariants {
    Event(CentralEvent),
    Notif(ValueNotification),
    GuiState(AppStateT),
}

pub enum EventStreams {
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
