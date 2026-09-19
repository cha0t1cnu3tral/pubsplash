//! Settings dialog for the built-in RNNoise suppressor.

use crate::config::NoiseSuppressorConfig;
use crate::t;
use std::rc::Rc;
use wxdragon::prelude::*;

pub fn edit(parent: &Dialog, current: &NoiseSuppressorConfig) -> Option<NoiseSuppressorConfig> {
    let dialog = Dialog::builder(parent, &t!("Noise suppressor"))
        .with_style(DialogStyle::DefaultDialogStyle)
        .with_size(500, 230)
        .build();
    let panel = Panel::builder(&dialog).build();
    let sizer = BoxSizer::builder(Orientation::Vertical).build();
    let label = t!("Suppression amount");
    let text = StaticText::builder(&panel).with_label(&label).build();
    let amount = Slider::builder(&panel)
        .with_value(current.amount.min(100) as i32)
        .with_min_value(0)
        .with_max_value(100)
        .build();
    super::set_accessible_name(&amount, &label);
    let announcer = Rc::new(super::slider_uia::install(&amount));
    announcer.set_text(&label, &t!("{value}%", value = current.amount.min(100)));
    {
        let announcer = announcer.clone();
        let label = label.clone();
        amount.on_slider(move |_| {
            announcer.update(&label, &t!("{value}%", value = amount.value().max(0)));
        });
    }
    {
        let announcer = announcer.clone();
        let label = label.clone();
        amount.on_key_down(move |event| {
            let Some((code, _)) = super::key_of(&event) else {
                event.skip(true);
                return;
            };
            let Some(value) = super::slider_uia::key_step(code, amount.value(), 0, 100, 10) else {
                event.skip(true);
                return;
            };
            event.skip(false);
            amount.set_value(value);
            announcer.update(&label, &t!("{value}%", value = value));
        });
    }
    super::help::tag(
        &amount,
        "dialog.noiseSuppressor.amount",
        "Noise suppressor amount slider",
    );
    sizer.add(&text, 0, SizerFlag::All, 4);
    sizer.add(&amount, 0, SizerFlag::Expand | SizerFlag::All, 4);
    let buttons = BoxSizer::builder(Orientation::Horizontal).build();
    let ok = super::ok_button(&panel, &t!("OK"));
    let cancel = Button::builder(&panel)
        .with_id(ID_CANCEL)
        .with_label(&t!("Cancel"))
        .build();
    buttons.add(&ok, 0, SizerFlag::All, 4);
    buttons.add(&cancel, 0, SizerFlag::All, 4);
    sizer.add_sizer(&buttons, 0, SizerFlag::AlignRight, 0);
    panel.set_sizer(sizer, true);
    let dialog_sizer = BoxSizer::builder(Orientation::Vertical).build();
    dialog_sizer.add(&panel, 1, SizerFlag::Expand, 0);
    dialog.set_sizer(dialog_sizer, true);
    ok.on_click(move |_| dialog.end_modal(ID_OK));
    cancel.on_click(move |_| dialog.end_modal(ID_CANCEL));
    let edited = (dialog.show_modal() == ID_OK).then(|| NoiseSuppressorConfig {
        amount: amount.value().clamp(0, 100) as u32,
    });
    announcer.uninstall();
    dialog.destroy();
    edited
}
