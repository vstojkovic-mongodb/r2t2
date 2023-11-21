mod chart;
mod layout;
mod main_window;
mod menu;

pub use main_window::{MainWindow, Update};

struct ScopedClip;

impl ScopedClip {
    pub fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        fltk::draw::push_clip(x, y, w, h);
        Self
    }
}

impl Drop for ScopedClip {
    fn drop(&mut self) {
        fltk::draw::pop_clip();
    }
}

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
