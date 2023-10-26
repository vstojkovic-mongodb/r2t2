use std::rc::Rc;

use fltk::app::{self, Sender};
use fltk::dialog::{FileDialogType, NativeFileChooser};
use fltk::enums::Shortcut;
use fltk::menu::MenuBar;
use fltk::prelude::*;
use fltk::window::Window;

use crate::Message;

use super::auto_size::AutoSizeExt;
use super::menu::MenuExt;
use super::weak_cb;

pub struct MainWindow {
    window: Window,
    tx: Sender<Message>,
}

impl MainWindow {
    pub fn new(width: i32, height: i32, tx: Sender<Message>) -> Rc<Self> {
        let (screen_x, screen_y, screen_w, screen_h) = app::Screen::work_area_mouse().tup();
        let x = screen_x + (screen_w - width) / 2;
        let y = screen_y + (screen_h - height) / 2;

        let mut window = Window::default()
            .with_label("r2t2")
            .with_pos(x, y)
            .with_size(width, height);
        window.size_range(1, 1, 0, 0);
        window.make_resizable(true);

        let mut menu = MenuBar::default();
        let mut open_item = menu.add_item("&File/_&Open...\t\t", Shortcut::Ctrl | 'o');
        let mut exit_item = menu.add_item("&File/E&xit\t\t", Shortcut::None);

        menu.resize(0, 0, window.w(), menu.min_h());

        let this = Rc::new(Self { window, tx });

        open_item.set_callback(weak_cb!(|this, _| this.on_open_file()));
        exit_item.set_callback(|_| app::quit());

        this
    }

    pub fn show(&self) {
        self.window.clone().show();
    }

    fn on_open_file(&self) {
        let mut dialog = NativeFileChooser::new(FileDialogType::BrowseFile);
        dialog.show();

        if let Some(filename) = dialog.filenames().first() {
            self.tx.send(Message::OpenFile(filename.clone()));
        }
    }
}
