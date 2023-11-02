use fltk::enums::Shortcut;

pub trait MenuConvenienceExt {
    fn add_item(&mut self, text: &str, shortcut: Shortcut) -> i32;
}

impl<M: fltk::prelude::MenuExt> MenuConvenienceExt for M {
    fn add_item(&mut self, text: &str, shortcut: Shortcut) -> i32 {
        let idx = self.add_choice(text);
        let mut item = self.at(idx).unwrap();
        item.set_shortcut(shortcut);
        idx
    }
}
