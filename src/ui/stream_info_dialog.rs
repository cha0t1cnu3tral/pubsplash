//! Set stream info dialog: title, description, and whether the stream is
//! archived on the server. Values live for this session only.

use super::App;
use crate::config::StreamFormat;
use crate::t;
use std::rc::Rc;
use wxdragon::prelude::*;

/// Encoder bitrates offered by the Quality dropdown, in kbps. These are the
/// discrete steps the stream encoders accept. The Audio Pub server imposes no
/// bitrate limit (it only checks the codec), so the full range is valid.
const QUALITY_KBPS: [u32; 8] = [48, 64, 96, 128, 160, 192, 256, 320];
const STREAM_FORMATS: [StreamFormat; 2] = [StreamFormat::Mp3, StreamFormat::Aac];

/// Shows the dialog pre-filled with the current session's stream info.
/// Returns true if the user confirmed with OK (info is then stored in
/// `Runtime::stream_info` and marked as set).
pub fn show(app: &Rc<App>, parent: &Frame) -> bool {
    let dialog = Dialog::builder(parent, &t!("Set stream info"))
        .with_style(DialogStyle::DefaultDialogStyle | DialogStyle::ResizeBorder)
        .with_size(480, 440)
        .build();
    let panel = Panel::builder(&dialog).build();
    let sizer = BoxSizer::builder(Orientation::Vertical).build();

    let current = app.run.borrow().stream_info.clone();

    let title_label = StaticText::builder(&panel).with_label(&t!("Title")).build();
    let title_input = TextCtrl::builder(&panel).with_value(&current.title).build();
    super::set_accessible_name(&title_input, &t!("Title"));
    super::help::tag(
        &title_input,
        "dialog.streamInfo.title",
        "Stream title input",
    );

    let description_label = StaticText::builder(&panel)
        .with_label(&t!("Description"))
        .build();
    let description_input = TextCtrl::builder(&panel)
        .with_style(TextCtrlStyle::MultiLine)
        .with_value(&current.description)
        .build();
    super::set_accessible_name(&description_input, &t!("Description"));
    super::help::tag(
        &description_input,
        "dialog.streamInfo.description",
        "Stream description input",
    );

    let quality_label = StaticText::builder(&panel)
        .with_label(&t!("Quality"))
        .build();
    let quality_choice = Choice::builder(&panel).build();
    super::set_accessible_name(&quality_choice, &t!("Quality"));
    super::help::tag(
        &quality_choice,
        "dialog.streamInfo.quality",
        "Stream audio quality (bitrate) choice",
    );
    for kbps in QUALITY_KBPS {
        quality_choice.append(&t!("{kbps} kbps", kbps = kbps));
    }
    // Persisted in config (unlike the session-only title/description above).
    let current_bitrate = app.config.borrow().audio.bitrate_kbps;
    let quality_index = QUALITY_KBPS
        .iter()
        .position(|&kbps| kbps == current_bitrate)
        .unwrap_or_else(|| QUALITY_KBPS.iter().position(|&kbps| kbps == 128).unwrap());
    quality_choice.set_selection(quality_index as u32);

    let format_label = StaticText::builder(&panel)
        .with_label(&t!("Stream format"))
        .build();
    let format_choice = Choice::builder(&panel).build();
    super::set_accessible_name(&format_choice, &t!("Stream format"));
    super::help::tag(
        &format_choice,
        "dialog.streamInfo.format",
        "Stream audio format choice",
    );
    for format in STREAM_FORMATS {
        format_choice.append(&format.display_name());
    }
    let current_format = app.config.borrow().audio.format;
    let format_index = STREAM_FORMATS
        .iter()
        .position(|&format| format == current_format)
        .unwrap_or(0);
    format_choice.set_selection(format_index as u32);

    let archive_check = CheckBox::builder(&panel)
        .with_label(&t!("Archive the stream"))
        .build();
    super::set_accessible_name(&archive_check, &t!("Archive the stream"));
    super::help::tag(
        &archive_check,
        "dialog.streamInfo.archive",
        "Archive the stream checkbox",
    );
    // Before the user confirms the dialog this session, the "Archive streams by
    // default" preference supplies the initial state; afterwards their explicit
    // choice (in `current.archive`) wins.
    let default_archive = current.archive
        || (!app.run.borrow().stream_info_set
            && app.config.borrow().archiving.archive_streams_by_default);
    archive_check.set_value(default_archive);

    let record_check = CheckBox::builder(&panel)
        .with_label(&t!("Record this stream"))
        .build();
    super::set_accessible_name(&record_check, &t!("Record this stream"));
    super::help::tag(
        &record_check,
        "dialog.streamInfo.record",
        "Record this stream locally checkbox",
    );
    // Same rule as archiving: the "Record streams by default" preference seeds
    // the box until the user confirms the dialog this session.
    let default_record = current.record
        || (!app.run.borrow().stream_info_set
            && app.config.borrow().archiving.record_streams_by_default);
    record_check.set_value(default_record);

    // Mastodon group. Its two boxes are seeded from Preferences the same way
    // the archive and record boxes above are, and decide on their own from
    // there: unchecking one here silences the announcements for this stream
    // without touching the saved defaults.
    let (mastodon_group, mastodon_box) = super::group_box(&panel, &t!("Mastodon"));
    let linked = app.config.borrow().mastodon.is_linked();

    let announce_start = CheckBox::builder(&mastodon_box)
        .with_label(&t!("Post to Mastodon when this stream starts"))
        .build();
    super::help::tag(
        &announce_start,
        "dialog.streamInfo.mastodonStart",
        "Post to Mastodon when this stream starts checkbox",
    );
    let announce_periodic = CheckBox::builder(&mastodon_box)
        .with_label(&t!("Post periodic still-streaming announcements"))
        .build();
    super::help::tag(
        &announce_periodic,
        "dialog.streamInfo.mastodonPeriodic",
        "Post periodic still-streaming announcements checkbox",
    );
    for (check, label, default) in [
        (
            &announce_start,
            &t!("Post to Mastodon when this stream starts"),
            current.announce_start
                || (!app.run.borrow().stream_info_set
                    && app.config.borrow().mastodon.post_on_start),
        ),
        (
            &announce_periodic,
            &t!("Post periodic still-streaming announcements"),
            current.announce_periodic
                || (!app.run.borrow().stream_info_set && app.config.borrow().mastodon.periodic),
        ),
    ] {
        check.set_value(linked && default);
        check.enable(linked);
        // The accessible name says *why* the box is unavailable, because a
        // disabled checkbox with no explanation is a dead end.
        super::set_accessible_name(
            check,
            &if linked {
                label.to_string()
            } else {
                t!(
                    "{label}, unavailable until a Mastodon account is linked in Preferences",
                    label = label
                )
            },
        );
    }
    mastodon_group.add(&announce_start, 0, SizerFlag::All, 4);
    mastodon_group.add(&announce_periodic, 0, SizerFlag::All, 4);

    let buttons = BoxSizer::builder(Orientation::Horizontal).build();
    let ok_button = super::ok_button(&panel, &t!("OK"));
    // `ID_CANCEL` is what wx maps Escape to; without it Escape does nothing.
    let cancel_button = Button::builder(&panel)
        .with_id(ID_CANCEL)
        .with_label(&t!("Cancel"))
        .build();
    buttons.add(&ok_button, 0, SizerFlag::All, 4);
    buttons.add(&cancel_button, 0, SizerFlag::All, 4);

    sizer.add(&title_label, 0, SizerFlag::All, 4);
    sizer.add(&title_input, 0, SizerFlag::Expand | SizerFlag::All, 4);
    sizer.add(&description_label, 0, SizerFlag::All, 4);
    sizer.add(&description_input, 1, SizerFlag::Expand | SizerFlag::All, 4);
    sizer.add(&quality_label, 0, SizerFlag::All, 4);
    sizer.add(&quality_choice, 0, SizerFlag::Expand | SizerFlag::All, 4);
    sizer.add(&format_label, 0, SizerFlag::All, 4);
    sizer.add(&format_choice, 0, SizerFlag::Expand | SizerFlag::All, 4);
    sizer.add(&archive_check, 0, SizerFlag::All, 8);
    sizer.add(&record_check, 0, SizerFlag::All, 8);
    sizer.add_sizer(&mastodon_group, 0, SizerFlag::Expand | SizerFlag::All, 4);
    sizer.add_sizer(&buttons, 0, SizerFlag::All, 4);
    panel.set_sizer(sizer, true);
    let dialog_sizer = BoxSizer::builder(Orientation::Vertical).build();
    dialog_sizer.add(&panel, 1, SizerFlag::Expand, 0);
    dialog.set_sizer(dialog_sizer, true);

    // Tabbing into a text field selects its contents so the pre-filled
    // defaults can be overwritten by just typing.
    for input in [&title_input, &description_input] {
        let input_for_focus = *input;
        input.on_set_focus(move |event| {
            input_for_focus.select_all();
            event.skip(true);
        });
    }

    {
        let app = app.clone();
        let dialog_for_ok = dialog;
        ok_button.on_click(move |_| {
            let mut title = title_input.get_value().trim().to_string();
            if title.is_empty() {
                // The server requires a title; fall back to the default.
                title = t!("Stream");
            }
            let mut run = app.run.borrow_mut();
            run.stream_info.title = title;
            run.stream_info.description = description_input.get_value().trim().to_string();
            run.stream_info.archive = archive_check.get_value();
            run.stream_info.record = record_check.get_value();
            run.stream_info.announce_start = announce_start.get_value();
            run.stream_info.announce_periodic = announce_periodic.get_value();
            run.stream_info_set = true;
            drop(run);

            // Quality persists in config, unlike the session-only fields above.
            let kbps = quality_choice
                .get_selection()
                .map(|i| QUALITY_KBPS[i as usize])
                .unwrap_or(128);
            let format = format_choice
                .get_selection()
                .and_then(|i| STREAM_FORMATS.get(i as usize))
                .copied()
                .unwrap_or(StreamFormat::Mp3);
            let mut config = app.config.borrow_mut();
            config.audio.bitrate_kbps = kbps;
            config.audio.format = format;
            drop(config);
            app.save_config();

            dialog_for_ok.end_modal(ID_OK);
        });
    }
    {
        let dialog_for_cancel = dialog;
        cancel_button.on_click(move |_| {
            dialog_for_cancel.end_modal(ID_CANCEL);
        });
    }

    let confirmed = dialog.show_modal() == ID_OK;
    dialog.destroy();
    confirmed
}
