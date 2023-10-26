use fltk::menu::MenuBar;
use fltk::prelude::*;

pub trait AutoSizeExt {
    fn min_w(&self) -> i32;
    fn min_h(&self) -> i32;
    fn min_size(&self) -> (i32, i32) {
        (self.min_w(), self.min_h())
    }
}

const MENU_ITEM_PAD_WIDTH: i32 = 16; // lifted from FLTK source

impl AutoSizeExt for MenuBar {
    fn min_w(&self) -> i32 {
        let frame = self.frame();
        let frame_dx = frame.dx();
        let frame_dw = frame.dw();
        let frame_w = frame_dx + frame_dw;

        let mut width = frame_w;

        let mut it = self.at(0);
        while let Some(item) = it {
            let (item_w, _) = item.measure();
            width += item_w + MENU_ITEM_PAD_WIDTH;
            it = item.next(1);
        }

        width
    }

    fn min_h(&self) -> i32 {
        let frame = self.frame();
        let frame_dy = frame.dy();
        let frame_dh = frame.dh();
        let frame_h = frame_dy + frame_dh;

        fltk::draw::set_font(self.text_font(), self.text_size());
        let font_h = fltk::draw::height();

        font_h + frame_h * 2
    }
}
