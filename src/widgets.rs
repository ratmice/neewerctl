use crate::{AnimMode, AppState, CCTMode, HSIMode, Light, LightMode};
use crate::{AppStateT, Changed};
use druid::piet::TextAlignment;
use druid::widget::{
    Button, CrossAxisAlignment, Either, Flex, Label, LensWrap, List, ListIter, MainAxisAlignment,
    Padding, Scroll, Slider, Spinner, Stepper, Switch, Widget, WidgetExt,
};
use druid::{Data, Env, LocalizedString};
use druid_widget_nursery::prism::{Closures};
use druid_widget_nursery::{MultiRadio, OnChange};

impl ListIter<Light> for AppState {
    fn for_each(&self, mut cb: impl FnMut(&Light, usize)) {
        for (i, (_k, v)) in self.lights.iter().enumerate() {
            cb(v, i);
        }
    }

    fn for_each_mut(&mut self, mut cb: impl FnMut(&mut Light, usize)) {
        let lights = std::sync::Arc::make_mut(&mut self.lights);
        for (i, (_k, v)) in lights.iter_mut().enumerate() {
            let mut ret = v.clone();
            cb(&mut ret, i);

            if !v.same(&ret) {
                v._changes_ |= ret._changes_;
                *v = ret;
            }
        }
    }

    fn data_len(&self) -> usize {
        self.lights.len()
    }
}

pub fn cct_widget() -> impl Widget<CCTMode> {
    let mut col_cct = Flex::column();
    let mut row_temp = Flex::row();
    let mut row_brightness = Flex::row();

    let slider_temp = LensWrap::new(
        Slider::new().with_range(32.0, 56.0).with_step(1.0),
        CCTMode::temp,
    );
    let stepper_temp = LensWrap::new(Stepper::new().with_range(32.0, 56.0), CCTMode::temp);

    let mut state_temp = Label::new(|d: &CCTMode, _: &Env| format!("{:>7.2}", d.temp));
    state_temp.set_text_alignment(TextAlignment::End);
    let mut label_temp = Label::new("temp");
    label_temp.set_text_alignment(TextAlignment::End);
    row_temp.add_flex_child(label_temp, 1.0);
    row_temp.add_flex_child(Padding::new(0.5, slider_temp), 1.0);
    row_temp.add_flex_child(stepper_temp, 1.0);
    row_temp.add_flex_child(state_temp, 1.0);

    col_cct.add_child(row_temp);
    let slider_brightness = LensWrap::new(
        Slider::new().with_range(0.0, 100.0).with_step(1.0),
        CCTMode::brightness,
    );

    let stepper_brightness =
        LensWrap::new(Stepper::new().with_range(0.0, 100.0), CCTMode::brightness);
    let state_brightness = Label::new(|d: &CCTMode, _: &Env| format!("{:>7.2}", d.brightness));
    let mut label = Label::new("brightness");
    label.set_text_alignment(TextAlignment::End);
    row_brightness.add_flex_child(label, 1.0);
    row_brightness.add_flex_child(Padding::new(0.5, slider_brightness), 1.0);
    row_brightness.add_flex_child(state_brightness, 1.0);
    row_brightness.add_flex_child(stepper_brightness, 1.0);
    col_cct.add_child(row_brightness);
    col_cct
}

pub fn hsi_widget() -> impl Widget<HSIMode> {
    let mut col_hsi = Flex::column();
    let mut row_h = Flex::row();
    let mut row_s = Flex::row();
    let mut row_i = Flex::row();
    let slider_h = LensWrap::new(
        Slider::new().with_range(0.0, 360.0).with_step(1.0),
        HSIMode::hue,
    );
    let stepper_h = LensWrap::new(Stepper::new().with_range(0.0, 360.0), HSIMode::hue);
    let state_h = Label::new(|d: &HSIMode, _: &Env| format!("{:>7.2}", d.hue));
    row_h.add_flex_child(Label::new("h"), 1.0);
    row_h.add_flex_child(Padding::new(0.5, slider_h), 1.0);
    row_h.add_flex_child(stepper_h, 1.0);
    row_h.add_flex_child(state_h, 1.0);
    col_hsi.add_child(row_h);

    let slider_s = LensWrap::new(
        Slider::new().with_range(0.0, 100.0).with_step(1.0),
        HSIMode::saturation,
    );
    let stepper_s = LensWrap::new(Stepper::new().with_range(0.0, 100.0), HSIMode::saturation);
    let state_s = Label::new(|d: &HSIMode, _: &Env| format!("{:>7.2}", d.saturation));

    row_s.add_flex_child(Label::new("s"), 1.0);
    row_s.add_flex_child(Padding::new(0.5, slider_s), 1.0);
    row_s.add_flex_child(stepper_s, 1.0);
    row_s.add_flex_child(state_s, 1.0);
    col_hsi.add_child(row_s);

    let slider_i = LensWrap::new(
        Slider::new().with_range(0.0, 100.0).with_step(1.0),
        HSIMode::intensity,
    );
    let stepper_i = LensWrap::new(Stepper::new().with_range(0.0, 100.0), HSIMode::intensity);
    let state_i = Label::new(|d: &HSIMode, _: &Env| format!("{:>7.2}", d.intensity));
    row_i.add_flex_child(Label::new("i"), 1.0);
    row_i.add_flex_child(Padding::new(0.5, slider_i), 1.0);
    row_i.add_flex_child(stepper_i, 1.0);
    row_i.add_flex_child(state_i, 1.0);
    col_hsi.add_child(row_i);
    col_hsi
}

pub fn scene_widget() -> impl Widget<AnimMode> {
    let mut col_scene = Flex::column();
    let mut row_brightness = Flex::row();
    let mut row_scene1 = Flex::row();
    /*
    let mut row_scene2 = Flex::row();
    let mut row_scene3 = Flex::row();
    */
    let stepper_scene = LensWrap::new(Stepper::new().with_range(0.0, 9.0), AnimMode::scene);
    row_scene1.add_flex_child(stepper_scene, 1.0);
    let slider_brightness = LensWrap::new(
        Slider::new().with_range(0.0, 100.0).with_step(1.0),
        AnimMode::brightness,
    );

    let stepper_brightness =
        LensWrap::new(Stepper::new().with_range(0.0, 100.0), AnimMode::brightness);
    let state_brightness = Label::new(|d: &AnimMode, _: &Env| format!("{:>7.2}", d.brightness));
    let mut label = Label::new("brightness");
    label.set_text_alignment(TextAlignment::End);
    row_brightness.add_flex_child(label, 1.0);
    row_brightness.add_flex_child(Padding::new(0.5, slider_brightness), 1.0);
    row_brightness.add_flex_child(state_brightness, 1.0);
    row_brightness.add_flex_child(stepper_brightness, 1.0);
    col_scene.add_child(row_brightness);
    col_scene.add_child(row_scene1);
    col_scene
}

pub fn device_widget() -> impl Widget<Light> {
    let mut col = Flex::column().main_axis_alignment(MainAxisAlignment::Start);
    let mut row_top = Flex::row().main_axis_alignment(MainAxisAlignment::Start);

    //let channel = tx_gui.clone();
    let switch = LensWrap::new(Switch::new(), Light::power).on_click(
        move |_ctxt, data: &mut Light, _env| {
            data.toggle_power();
        },
    );

    /*  let name_label = Label::new(|d: &Light, _: &Env| format!("{}", d.name));
    row_top.add_child(name_label);*/
    //    let mut state_temp = Label::new(|d: &CCTMode, _: &Env| format!("{:>7.2}", d.temp));
    let switch_label = Label::new("Power:");
    row_top.add_child(Padding::new(5.0, switch_label));
    row_top.add_child(Padding::new(5.0, switch));

    col.set_main_axis_alignment(MainAxisAlignment::Center);
    col.set_cross_axis_alignment(CrossAxisAlignment::Start);
    col.add_child(Padding::new(0.0, row_top));

    let a = MultiRadio::new(
        "CCT",
        cct_widget(),
        CCTMode {
            temp: 0.0,
            brightness: 0.0,
        },
        Closures(
            |outer: &Light| {
                if let LightMode::CCT(ref value) = outer.mode {
                    Some(value.clone())
                } else {
                    None
                }
            },
            move |data: &mut Light, inner: CCTMode| {
                data.mode = LightMode::CCT(inner);
            },
        ),
    )
    .controller(OnChange::new(move |_, _: &Light, data, _| {
        data._changes_ |= Changed::Mode as u8;
    }));

    //let channel = tx_gui.clone();
    let b = MultiRadio::new(
        "HSI",
        hsi_widget(),
        HSIMode {
            hue: 0.0,
            saturation: 0.0,
            intensity: 0.0,
        },
        Closures(
            |outer: &Light| {
                if let LightMode::HSI(ref value) = outer.mode {
                    Some(value.clone())
                } else {
                    None
                }
            },
            move |data: &mut Light, inner: HSIMode| {
                data.mode = LightMode::HSI(inner);
            },
        ),
    )
    .controller(OnChange::new(move |_, _: &Light, data, _| {
        data._changes_ |= Changed::Mode as u8;
    }));

    //let channel = tx_gui.clone();
    let c = MultiRadio::new(
        "Anim",
        scene_widget(),
        AnimMode {
            scene: 1.0,
            brightness: 0.0,
        },
        Closures(
            |outer: &Light| {
                if let LightMode::Anim(ref value) = outer.mode {
                    Some(value.clone())
                } else {
                    None
                }
            },
            move |data: &mut Light, inner: AnimMode| {
                data.mode = LightMode::Anim(inner);
            },
        ),
    )
    .controller(OnChange::new(move |_, _: &Light, data, _| {
        data._changes_ |= Changed::Mode as u8;
    }));

    col.add_child(a);
    col.add_child(b);
    col.add_child(c);
    col.center()
}

pub fn devices_widget() -> impl Widget<AppStateT> {
    let mut lists = Flex::row().cross_axis_alignment(CrossAxisAlignment::Start);
    let list = List::new(device_widget);
    lists.add_flex_child(
        Scroll::new(list).vertical().lens(druid::lens::Identity),
        1.0,
    );
    let button = Button::new("Scan").on_click(move |_, data: &mut AppStateT, _env| {
        data.toggle_scanning();
    });

    let button_placeholder = Flex::row()
        .cross_axis_alignment(CrossAxisAlignment::Start)
        .with_child(Label::new(LocalizedString::new("Scanning")))
        .with_child(Spinner::new());
    let either = Either::new(|data, _env| data.scanning, button_placeholder, button);

    Flex::column()
        .cross_axis_alignment(CrossAxisAlignment::Start)
        .with_child(either)
        .with_child(lists)
}
