use fltk::enums::Shortcut;
use fltk::menu::MenuItem;

pub trait MenuExt {
    fn add_item(&mut self, text: &str, shortcut: Shortcut) -> MenuItem;
}

impl<M: fltk::prelude::MenuExt> MenuExt for M {
    fn add_item(&mut self, text: &str, shortcut: Shortcut) -> MenuItem {
        let idx = self.add_choice(text);
        let mut item = self.at(idx).unwrap();
        item.set_shortcut(shortcut);
        item
    }
}
