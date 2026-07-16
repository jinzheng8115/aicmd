use super::{MarkdownRender, SseEvent};

use crate::utils::{poll_abort_signal, spawn_progress_spinner, AbortSignal};

use anyhow::Result;
use crossterm::{
    cursor, queue, style,
    terminal::{self, disable_raw_mode, enable_raw_mode},
};
use std::{
    io::{self, stdout, Stdout, Write},
    time::Duration,
};
use textwrap::core::display_width;
use tokio::sync::mpsc::UnboundedReceiver;

struct RawModeGuard {
    enabled: bool,
}

impl RawModeGuard {
    fn enable() -> Result<Self> {
        enable_raw_mode()?;
        Ok(Self { enabled: true })
    }

    fn finish(mut self) -> Result<()> {
        if self.enabled {
            disable_raw_mode()?;
            self.enabled = false;
        }
        Ok(())
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        if self.enabled {
            let _ = disable_raw_mode();
        }
    }
}

pub async fn markdown_stream(
    rx: UnboundedReceiver<SseEvent>,
    render: &mut MarkdownRender,
    abort_signal: &AbortSignal,
    progress: (String, usize, usize, bool),
) -> Result<()> {
    let raw_mode = RawModeGuard::enable()?;
    let mut stdout = io::stdout();

    let ret = markdown_stream_inner(rx, render, abort_signal, &mut stdout, progress).await;

    raw_mode.finish()?;

    if ret.is_err() {
        println!();
    }
    ret
}

pub async fn raw_stream(
    mut rx: UnboundedReceiver<SseEvent>,
    abort_signal: &AbortSignal,
    progress: (String, usize, usize, bool),
) -> Result<()> {
    let (stage, attempt, max_attempts, chinese) = progress;
    let mut spinner = Some(spawn_progress_spinner(
        stage,
        attempt,
        max_attempts,
        chinese,
    ));

    loop {
        if abort_signal.aborted() {
            break;
        }
        let Some(evt) = rx.recv().await else {
            break;
        };
        if let Some(spinner) = spinner.take() {
            spinner.stop();
        }

        match evt {
            SseEvent::Text(text) => {
                print!("{text}");
                stdout().flush()?;
            }
            SseEvent::Done => {
                break;
            }
        }
    }
    if let Some(spinner) = spinner.take() {
        spinner.stop();
    }
    Ok(())
}

async fn markdown_stream_inner(
    mut rx: UnboundedReceiver<SseEvent>,
    render: &mut MarkdownRender,
    abort_signal: &AbortSignal,
    writer: &mut Stdout,
    progress: (String, usize, usize, bool),
) -> Result<()> {
    let mut buffer = String::new();
    let mut buffer_rows = 1;

    let columns = terminal::size()?.0;

    let (stage, attempt, max_attempts, chinese) = progress;
    let mut spinner = Some(spawn_progress_spinner(
        stage,
        attempt,
        max_attempts,
        chinese,
    ));

    'outer: loop {
        if abort_signal.aborted() {
            break;
        }
        for reply_event in gather_events(&mut rx).await {
            if let Some(spinner) = spinner.take() {
                spinner.stop();
            }

            match reply_event {
                SseEvent::Text(mut text) => {
                    // tab width hacking
                    text = text.replace('\t', "    ");

                    let mut attempts = 0;
                    let (col, mut row) = loop {
                        match cursor::position() {
                            Ok(pos) => break pos,
                            Err(_) if attempts < 3 => attempts += 1,
                            Err(e) => return Err(e.into()),
                        }
                    };

                    // Fix unexpected duplicate lines on kitty, see https://github.com/sigoden/aichat/issues/105
                    if col == 0 && row > 0 && display_width(&buffer) == columns as usize {
                        row -= 1;
                    }

                    if row + 1 >= buffer_rows {
                        queue!(writer, cursor::MoveTo(0, row + 1 - buffer_rows),)?;
                    } else {
                        let scroll_rows = buffer_rows - row - 1;
                        queue!(
                            writer,
                            terminal::ScrollUp(scroll_rows),
                            cursor::MoveTo(0, 0),
                        )?;
                    }

                    // No guarantee that text returned by render will not be re-layouted, so it is better to clear it.
                    queue!(writer, terminal::Clear(terminal::ClearType::FromCursorDown))?;

                    if text.contains('\n') {
                        let text = format!("{buffer}{text}");
                        let (head, tail) = split_line_tail(&text);
                        let output = render.render(head);
                        print_block(writer, &output, columns)?;
                        buffer = tail.to_string();
                    } else {
                        buffer = format!("{buffer}{text}");
                    }

                    let output = render.render_line(&buffer);
                    if output.contains('\n') {
                        let (head, tail) = split_line_tail(&output);
                        buffer_rows = print_block(writer, head, columns)?;
                        queue!(writer, style::Print(&tail),)?;

                        // No guarantee the buffer width of the buffer will not exceed the number of columns.
                        // So we calculate the number of rows needed, rather than setting it directly to 1.
                        buffer_rows += need_rows(tail, columns);
                    } else {
                        queue!(writer, style::Print(&output))?;
                        buffer_rows = need_rows(&output, columns);
                    }

                    writer.flush()?;
                }
                SseEvent::Done => {
                    break 'outer;
                }
            }
        }

        if poll_abort_signal(abort_signal)? {
            break;
        }
    }

    if let Some(spinner) = spinner.take() {
        spinner.stop();
    }
    Ok(())
}

async fn gather_events(rx: &mut UnboundedReceiver<SseEvent>) -> Vec<SseEvent> {
    let mut texts = vec![];
    let mut done = false;
    tokio::select! {
        _ = async {
            while let Some(reply_event) = rx.recv().await {
                match reply_event {
                    SseEvent::Text(v) => texts.push(v),
                    SseEvent::Done => {
                        break;
                    }
                }
            }
            done = true;
        } => {}
        _ = tokio::time::sleep(Duration::from_millis(50)) => {}
    };
    let mut events = vec![];
    if !texts.is_empty() {
        events.push(SseEvent::Text(texts.join("")))
    }
    if done {
        events.push(SseEvent::Done)
    }
    events
}

fn print_block(writer: &mut Stdout, text: &str, columns: u16) -> Result<u16> {
    let mut num = 0;
    for line in text.split('\n') {
        queue!(
            writer,
            style::Print(line),
            style::Print("\n"),
            cursor::MoveLeft(columns),
        )?;
        num += 1;
    }
    Ok(num)
}

fn split_line_tail(text: &str) -> (&str, &str) {
    if let Some((head, tail)) = text.rsplit_once('\n') {
        (head, tail)
    } else {
        ("", text)
    }
}

fn need_rows(text: &str, columns: u16) -> u16 {
    let buffer_width = display_width(text).max(1) as u16;
    buffer_width.div_ceil(columns)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::create_abort_signal;
    use tokio::sync::mpsc::unbounded_channel;

    #[tokio::test]
    async fn raw_stream_returns_when_sender_closes_without_done_event() {
        let (tx, rx) = unbounded_channel();
        drop(tx);

        let result = tokio::time::timeout(
            Duration::from_millis(200),
            raw_stream(
                rx,
                &create_abort_signal(),
                ("Generating command".to_string(), 1, 3, false),
            ),
        )
        .await;

        assert!(
            result.is_ok(),
            "raw stream must not wait forever after sender closes"
        );
        assert!(result.unwrap().is_ok());
    }

    #[tokio::test]
    async fn gather_events_turns_closed_sender_into_done_event() {
        let (tx, mut rx) = unbounded_channel();
        drop(tx);

        let events = gather_events(&mut rx).await;

        assert!(matches!(events.as_slice(), [SseEvent::Done]));
    }
}
