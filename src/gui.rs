mod layout;
mod main_window;
mod menu;

pub use main_window::{MainWindow, Update};

macro_rules! weak_cb {
    (|$this:ident $(, $arg_id:tt)*| $body:expr) => {
        {
            let $this = std::rc::Rc::downgrade(&$this);
            move |$($arg_id),*| {
                if let Some($this) = $this.upgrade() {
                    $body
                }
            }
        }
    };
}
pub(crate) use weak_cb;
