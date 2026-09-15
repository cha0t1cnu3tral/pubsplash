//! Settings dialog for one built-in compressor.

use super::slider_uia::SliderAnnouncer;
use crate::config::CompressorConfig;
use crate::t;
use std::rc::Rc;
use wxdragon::prelude::*;

struct Row {
    slider: Slider,
    announcer: Rc<SliderAnnouncer>,
}

pub fn edit(parent: &Dialog, current: &CompressorConfig) -> Option<CompressorConfig> {
    let dialog = Dialog::builder(parent, &t!("Compressor"))
        .with_style(DialogStyle::DefaultDialogStyle)
        .with_size(520, 540)
        .build();
    let panel = Panel::builder(&dialog).build();
    let sizer = BoxSizer::builder(Orientation::Vertical).build();
    let mut rows = Vec::new();

    let mut slider = |label: &str, value: u32, min: u32, max: u32, page: i32, unit: Unit| {
        let text = StaticText::builder(&panel).with_label(label).build();
        let control = Slider::builder(&panel)
            .with_value(value.clamp(min, max) as i32)
            .with_min_value(min as i32)
            .with_max_value(max as i32)
            .build();
        super::set_accessible_name(&control, label);
        let announcer = Rc::new(super::slider_uia::install(&control));
        let name = label.to_string();
        announcer.set_text(&name, &unit.format(value.clamp(min, max)));
        {
            let announcer = announcer.clone();
            let name = name.clone();
            control.on_slider(move |_| {
                announcer.update(&name, &unit.format(control.value().max(0) as u32));
            });
        }
        {
            let announcer = announcer.clone();
            let name = name.clone();
            control.on_key_down(move |event| {
                let Some((code, _)) = super::key_of(&event) else {
                    event.skip(true);
                    return;
                };
                let Some(value) = super::slider_uia::key_step(
                    code,
                    control.value(),
                    min as i32,
                    max as i32,
                    page,
                ) else {
                    event.skip(true);
                    return;
                };
                event.skip(false);
                control.set_value(value);
                announcer.update(&name, &unit.format(value.max(0) as u32));
            });
        }
        sizer.add(&text, 0, SizerFlag::All, 4);
        sizer.add(&control, 0, SizerFlag::Expand | SizerFlag::All, 4);
        rows.push(Row {
            slider: control,
            announcer,
        });
        control
    };

    let threshold = slider(
        &t!("Start compressing at"),
        current.threshold,
        1,
        100,
        5,
        Unit::Percent,
    );
    let ratio = slider(
        &t!("Compression ratio"),
        current.ratio,
        1,
        20,
        1,
        Unit::Ratio,
    );
    let attack = slider(&t!("Attack"), current.attack_ms, 0, 500, 10, Unit::Millis);
    let release = slider(
        &t!("Release"),
        current.release_ms,
        0,
        3_000,
        100,
        Unit::Millis,
    );
    let output_gain = slider(
        &t!("Output gain"),
        current.output_gain,
        0,
        500,
        10,
        Unit::Percent,
    );

    super::help::tag(
        &threshold,
        "dialog.compressor.threshold",
        "Compressor threshold slider",
    );
    super::help::tag(&ratio, "dialog.compressor.ratio", "Compressor ratio slider");
    super::help::tag(
        &attack,
        "dialog.compressor.attack",
        "Compressor attack slider",
    );
    super::help::tag(
        &release,
        "dialog.compressor.release",
        "Compressor release slider",
    );
    super::help::tag(
        &output_gain,
        "dialog.compressor.outputGain",
        "Compressor output gain slider",
    );

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

    let edited = (dialog.show_modal() == ID_OK).then(|| CompressorConfig {
        threshold: threshold.value().max(1) as u32,
        ratio: ratio.value().max(1) as u32,
        attack_ms: attack.value().max(0) as u32,
        release_ms: release.value().max(0) as u32,
        output_gain: output_gain.value().max(0) as u32,
    });
    for row in &rows {
        let _ = row.slider;
        row.announcer.uninstall();
    }
    dialog.destroy();
    edited
}

#[derive(Clone, Copy)]
enum Unit {
    Percent,
    Ratio,
    Millis,
}

impl Unit {
    fn format(self, value: u32) -> String {
        match self {
            Unit::Percent => t!("{value}%", value = value),
            Unit::Ratio => t!("{value} to 1", value = value),
            Unit::Millis => t!("{value} ms", value = value),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn units_are_spoken_in_their_real_scale() {
        assert_eq!(Unit::Percent.format(20), "20%");
        assert_eq!(Unit::Ratio.format(4), "4 to 1");
        assert_eq!(Unit::Millis.format(150), "150 ms");
    }
}
