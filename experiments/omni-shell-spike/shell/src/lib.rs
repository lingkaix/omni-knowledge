//! Native shell spike.
//!
//! A native sidebar lists fixture mounts. A WebView island loads the shared
//! `@openkit/knowledge-ui` browser and talks to the fake client over NSH's
//! string channel. This crate is not the product GUI and it is not a service.

mod http;
mod session;

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use nsh_base::theme;
use nsh_core::desktop::{MenuRole, WindowFrame};
use nsh_core::element::desktop::{
    MenuItem, close_window, set_application_menu, set_frame, set_title,
};
use nsh_core::element::input;
use nsh_core::file_dialog::OpenDialog;
use nsh_core::prelude::*;

use crate::session::{Mode, Session};

/// Components a TSX entry could import. The packaged root is [`Shell`].
#[native_module("omni-shell-spike/native")]
fn native(registry: &mut Registry) {
    registry.register::<Shell>();
}

/// The spike window: native mount list beside the knowledge-ui island.
#[component]
pub fn Shell() -> impl IntoElement {
    let port = http::serve();
    eprintln!("spike-listening {port}");
    let session = Session::start(port);
    let mounts = session.borrow().mounts();
    install_menu(Rc::clone(&session));
    if session.borrow().mode() == Mode::Dialog {
        open_dialog(Rc::clone(&session));
    }
    arm(Rc::clone(&session));
    set_title("Omni Shell Spike");
    set_frame(WindowFrame {
        x: Some(px(40.0)),
        y: Some(px(40.0)),
        width: Some(px(1100.0)),
        height: Some(px(720.0)),
        animated: Some(false),
    });
    let selected = signal(
        mounts
            .first()
            .map(|(id, _)| id.clone())
            .unwrap_or_else(|| "m-library".to_owned()),
    );
    let web = element_ref();
    let page = format!("http://127.0.0.1:{port}/");
    view()
        .flex_1()
        .flex_row()
        .bg(move || theme().background)
        .text_color(move || theme().foreground)
        .on_file_drop({
            let session = Rc::clone(&session);
            move |event: &FileDropEvent| {
                session.borrow_mut().file_drop(&event.uris);
            }
        })
        .child(sidebar(mounts, selected, web, Rc::clone(&session)))
        .child(web_view(page).element_ref(&web).flex_1().on_message({
            let session = Rc::clone(&session);
            move |data: &str| {
                let outgoing = session.borrow_mut().on_page(data);
                for message in outgoing {
                    web.post_message(message);
                }
            }
        }))
}

fn sidebar(
    mounts: Vec<(String, String)>,
    selected: Signal<String>,
    web: ElementRef,
    session: Rc<RefCell<Session>>,
) -> impl IntoElement {
    let mut column = view()
        .w(px(240.0))
        .h_full()
        .p_3()
        .gap_2()
        .bg(move || theme().muted)
        .child(text("Mounts").text_size(px(18.0)))
        .child(text("Native sidebar").text_size(px(12.0)));
    for (id, title) in mounts {
        let row_id = id.clone();
        let mark_id = id.clone();
        let row_title = title.clone();
        let session = Rc::clone(&session);
        column = column.child(
            view()
                .py_2()
                .px_2()
                .rounded(px(8.0))
                .bg(move || theme().card)
                .on_press({
                    let row_id = row_id.clone();
                    let session = Rc::clone(&session);
                    move || {
                        selected.set(row_id.clone());
                        session.borrow_mut().sidebar_pressed();
                        web.post_message(
                            serde_json::json!({
                                "kind": "host",
                                "op": "set-parent",
                                "parentId": row_id,
                            })
                            .to_string(),
                        );
                    }
                })
                .child(text(move || {
                    let mark = if selected.get() == mark_id {
                        "● "
                    } else {
                        "○ "
                    };
                    format!("{mark}{row_title}")
                })),
        );
    }
    column.child(
        input()
            .placeholder("Native IME")
            .on_change({
                let session = Rc::clone(&session);
                move |text: &str| session.borrow_mut().native_ime(text)
            })
            .on_composition_change({
                let session = Rc::clone(&session);
                move |event: &Composition| {
                    session
                        .borrow_mut()
                        .native_composition(event.composing, &event.text);
                }
            }),
    )
}

fn install_menu(session: Rc<RefCell<Session>>) {
    session.borrow_mut().menu_installed();
    let record = Rc::clone(&session);
    let dialog = Rc::clone(&session);
    set_application_menu(vec![MenuItem::submenu(
        "File",
        [
            MenuItem::new("Open File")
                .shortcut("mod+o")
                .on_select(move || open_dialog(Rc::clone(&dialog))),
            MenuItem::separator(),
            MenuItem::new("Record Menu")
                .shortcut("mod+shift+m")
                .on_select(move || record.borrow_mut().menu_selected()),
            MenuItem::separator(),
            MenuItem::role(MenuRole::Quit),
        ],
    )]);
}

fn open_dialog(session: Rc<RefCell<Session>>) {
    let picked = pick_paths(OpenDialog::files().button_label("Open"));
    spawn(async move {
        let text = match picked.await {
            Ok(Some(paths)) => format!("ok {}", paths.len()),
            Ok(None) => "cancelled".to_owned(),
            Err(error) => format!("error {error}"),
        };
        session.borrow_mut().dialog_result(&text);
    });
}

fn arm(session: Rc<RefCell<Session>>) {
    let again = Rc::clone(&session);
    set_timeout(Duration::from_millis(500), move || {
        let due = again.borrow().due();
        if due {
            again.borrow_mut().write();
            close_window();
            return;
        }
        if again.borrow().mode() != Mode::Serve {
            arm(again);
        }
    });
}
