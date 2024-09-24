use ratatui::{buffer::Buffer, layout::Rect, style::{Color, Style}, widgets::{Block, Borders, Paragraph, StatefulWidget, Widget}, Frame};
use tui_input::Input;



pub(crate) struct SimpleTextInput<'a> {
    input: &'a Input,
    hide: bool,
    title: &'a str,
    
}



impl <'a> SimpleTextInput<'a> {
    pub fn new(title: &'a str, input: &'a Input, hide: bool) -> Self {
        Self {
            input,
            hide,
            title,
        }
    }

}


impl<'a> Widget for SimpleTextInput<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let scroll = self.input.visual_scroll(area.width as usize);

        let input_value = self
            .input
            .value()
            .chars()
            .map(|c| if self.hide { '*' } else { c })
            .collect::<String>();

        let text_field = Paragraph::new(input_value)
            .style(Style::default().fg(Color::Yellow))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(self.title)
                    .style(Style::default().fg(Color::White)),
            )
            .scroll((0, scroll as u16));

        text_field.render(area, buf);

        // if self.active {
        //     let cursor_x = area.x + 1 + (self.input.visual_cursor().min(width - 2)) as u16;
        //     let cursor_y = area.y + 1;
        //     // (cursor_x, cursor_y);
        // }
    }
}