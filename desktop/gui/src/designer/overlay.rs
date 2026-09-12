//! Interactive layer drawn over the pad preview: widget outlines, selection, resize handle,
//! and mouse input translated into pad coordinates.

use iced::mouse;
use iced::widget::canvas::{self, Action, Event, Frame, Geometry, Path, Stroke};
use iced::{Color, Point, Rectangle, Renderer, Size, Theme};

use super::{Message, HANDLE, SCALE};

pub struct Overlay {
    /// Widget boxes in pad pixels, in draw order
    pub boxes: Vec<(i16, i16, i16, i16)>,
    pub selected: Option<usize>,
}

#[derive(Default)]
pub struct State {
    pressed: bool,
}

fn pad_point(bounds: Rectangle, cursor: mouse::Cursor) -> Option<Point> {
    cursor.position_in(bounds).map(|p| Point::new(p.x / SCALE, p.y / SCALE))
}

impl canvas::Program<Message> for Overlay {
    type State = State;

    fn update(
        &self,
        state: &mut State,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<Action<Message>> {
        let Event::Mouse(mouse_event) = event else {
            return None;
        };
        let message = match mouse_event {
            mouse::Event::ButtonPressed(mouse::Button::Left) => {
                let p = pad_point(bounds, cursor)?;
                state.pressed = true;
                Message::PointerDown { x: p.x, y: p.y }
            }
            mouse::Event::CursorMoved { position } if state.pressed => {
                // Keep dragging even when the cursor leaves the preview
                let p = Point::new((position.x - bounds.x) / SCALE, (position.y - bounds.y) / SCALE);
                Message::PointerMove { x: p.x, y: p.y }
            }
            mouse::Event::ButtonReleased(mouse::Button::Left) if state.pressed => {
                state.pressed = false;
                Message::PointerUp
            }
            _ => return None,
        };
        Some(Action::publish(message).and_capture())
    }

    fn draw(
        &self,
        _state: &State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let scaled = |(x, y, w, h): (i16, i16, i16, i16)| {
            (Point::new(x as f32 * SCALE, y as f32 * SCALE), Size::new(w as f32 * SCALE, h as f32 * SCALE))
        };

        for (i, b) in self.boxes.iter().enumerate() {
            if Some(i) == self.selected {
                continue;
            }
            let (pos, size) = scaled(*b);
            frame.stroke(
                &Path::rectangle(pos, size),
                Stroke::default().with_color(Color::from_rgba(1.0, 1.0, 1.0, 0.18)).with_width(1.0),
            );
        }

        if let Some(b) = self.selected.and_then(|i| self.boxes.get(i)) {
            let (pos, size) = scaled(*b);
            let pink = Color::from_rgb8(0xFF, 0x66, 0xAA);
            frame.stroke(&Path::rectangle(pos, size), Stroke::default().with_color(pink).with_width(2.0));
            let handle = HANDLE as f32 * SCALE;
            frame.fill_rectangle(
                Point::new(pos.x + size.width - handle / 2.0, pos.y + size.height - handle / 2.0),
                Size::new(handle, handle),
                pink,
            );
        }
        vec![frame.into_geometry()]
    }

    fn mouse_interaction(&self, state: &State, bounds: Rectangle, cursor: mouse::Cursor) -> mouse::Interaction {
        let Some(p) = pad_point(bounds, cursor) else {
            return mouse::Interaction::default();
        };
        if state.pressed {
            return mouse::Interaction::Grabbing;
        }
        if let Some((x, y, w, h)) = self.selected.and_then(|i| self.boxes.get(i).copied()) {
            let (hx, hy) = ((x + w) as f32, (y + h) as f32);
            if (p.x - hx).abs() <= HANDLE as f32 && (p.y - hy).abs() <= HANDLE as f32 {
                return mouse::Interaction::ResizingDiagonallyDown;
            }
        }
        let over = self.boxes.iter().any(|(x, y, w, h)| {
            p.x >= *x as f32 && p.x < (*x + *w) as f32 && p.y >= *y as f32 && p.y < (*y + *h) as f32
        });
        if over { mouse::Interaction::Grab } else { mouse::Interaction::default() }
    }
}
