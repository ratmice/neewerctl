use crate::AppStateT;
use druid::{widget::Flex, Widget, WidgetExt};

pub fn devices_widget() -> impl Widget<AppStateT> {
    let col = Flex::column();

    col.center()
}
