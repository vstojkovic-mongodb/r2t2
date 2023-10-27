use std::rc::Rc;

use fltk::button::Button;
use fltk::frame::Frame;
use fltk::input::Input;
use fltk::menu::MenuBar;
use fltk::prelude::*;
use fltk_float::button::ButtonElement;
use fltk_float::frame::FrameElement;
use fltk_float::input::InputElement;
use fltk_float::{LayoutElement, LayoutWidgetWrapper, Size, WrapperFactory};

pub fn wrapper_factory() -> Rc<WrapperFactory> {
    WRAPPER_FACTORY.with(|factory| Rc::clone(factory))
}

pub struct MenuBarElement {
    widget: MenuBar,
}

impl LayoutWidgetWrapper<MenuBar> for MenuBarElement {
    fn wrap(widget: MenuBar) -> Self {
        Self { widget }
    }
}

impl LayoutElement for MenuBarElement {
    fn min_size(&self) -> Size {
        let frame = self.widget.frame();
        let frame_w = frame.dx() + frame.dw();
        let frame_h = frame.dy() + frame.dh();

        fltk::draw::set_font(self.widget.text_font(), self.widget.text_size());
        let mut width = frame_w;
        let mut height = fltk::draw::height();

        let mut it = self.widget.at(0);
        while let Some(item) = it {
            let (item_w, item_h) = item.measure();
            width += item_w + MENU_ITEM_PAD_WIDTH;
            height = std::cmp::max(height, item_h);
            it = item.next(1);
        }

        height += frame_h * 2;

        Size { width, height }
    }

    fn layout(&self, x: i32, y: i32, width: i32, height: i32) {
        self.widget.clone().resize(x, y, width, height);
    }
}

const MENU_ITEM_PAD_WIDTH: i32 = 16; // lifted from FLTK source

thread_local! {
    static WRAPPER_FACTORY: Rc<WrapperFactory> = {
        let mut factory = WrapperFactory::new();
        factory.set_wrapper::<Button, ButtonElement<Button>>();
        factory.set_wrapper::<Frame, FrameElement>();
        factory.set_wrapper::<Input, InputElement<Input>>();
        factory.set_wrapper::<MenuBar, MenuBarElement>();
        Rc::new(factory)
    }
}
