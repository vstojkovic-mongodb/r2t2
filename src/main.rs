use std::path::PathBuf;

use fltk::app;

mod ftdc;
mod gui;

use self::gui::MainWindow;

#[derive(Debug)]
pub enum Message {
    OpenFile(PathBuf),
}

fn main() {
    let app = app::App::default();
    let (tx, rx) = app::channel();

    let main_window = MainWindow::new(1280, 720, tx);

    app::add_check(move |_| {
        while let Some(msg) = rx.recv() {
            println!("{:?}", msg);
        }
    });

    main_window.show();
    app.run().unwrap();
}
