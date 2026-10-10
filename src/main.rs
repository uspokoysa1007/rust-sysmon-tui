use std::io::{self, stdout};
use std::time::{Duration, Instant};

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Row, Table},
    Terminal,
};
use sysinfo::{Disks, System};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Настройка терминала
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut sys = System::new_all();
    let mut disks = Disks::new_with_refreshed_list();

    let tick_rate = Duration::from_millis(500);
    let mut last_tick = Instant::now();

    loop {
        // Обновление метрик
        sys.refresh_all();
        disks.refresh(true);

        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([
                    Constraint::Length(3), // Заголовок
                    Constraint::Length(6), // CPU и RAM Gauge
                    Constraint::Min(10),   // Процессы и Диски
                    Constraint::Length(3), // Подвал с подсказками
                ])
                .split(f.area());

            // 1. Заголовок
            let header = Paragraph::new("⚡ RUST SYSTEM MONITOR (TUI) ⚡")
                .style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )
                .alignment(ratatui::layout::Alignment::Center)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::DarkGray)),
                );
            f.render_widget(header, chunks[0]);

            // 2. CPU & RAM
            let gauge_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(chunks[1]);

            let cpu_usage = sys.global_cpu_usage();
            let cpu_gauge = Gauge::default()
                .block(Block::default().title(" CPU Usage ").borders(Borders::ALL))
                .gauge_style(
                    Style::default()
                        .fg(if cpu_usage > 85.0 {
                            Color::Red
                        } else if cpu_usage > 50.0 {
                            Color::Yellow
                        } else {
                            Color::Green
                        })
                        .bg(Color::DarkGray),
                )
                .percent(cpu_usage.clamp(0.0, 100.0) as u16)
                .label(format!("{:.1}%", cpu_usage));
            f.render_widget(cpu_gauge, gauge_chunks[0]);

            let total_mem = sys.total_memory() as f64 / 1024.0 / 1024.0 / 1024.0;
            let used_mem = sys.used_memory() as f64 / 1024.0 / 1024.0 / 1024.0;
            let mem_percent = if total_mem > 0.0 {
                (used_mem / total_mem * 100.0).clamp(0.0, 100.0)
            } else {
                0.0
            };

            let mem_gauge = Gauge::default()
                .block(Block::default().title(" RAM Usage ").borders(Borders::ALL))
                .gauge_style(
                    Style::default()
                        .fg(if mem_percent > 85.0 {
                            Color::Red
                        } else {
                            Color::Magenta
                        })
                        .bg(Color::DarkGray),
                )
                .percent(mem_percent as u16)
                .label(format!("{:.2} GB / {:.2} GB ({:.1}%)", used_mem, total_mem, mem_percent));
            f.render_widget(mem_gauge, gauge_chunks[1]);

            // 3. Диски и Процессы
            let body_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
                .split(chunks[2]);

            // Диски
            let mut disk_items = Vec::new();
            for disk in disks.iter() {
                let total_gb = disk.total_space() as f64 / 1024.0 / 1024.0 / 1024.0;
                let free_gb = disk.available_space() as f64 / 1024.0 / 1024.0 / 1024.0;
                let used_gb = total_gb - free_gb;
                let pct = if total_gb > 0.0 {
                    (used_gb / total_gb * 100.0) as u16
                } else {
                    0
                };
                let mount = disk.mount_point().to_string_lossy();
                let line = Line::from(vec![
                    Span::styled(format!("💾 {}: ", mount), Style::default().fg(Color::Yellow)),
                    Span::raw(format!("{:.1}/{:.1} GB ({}%)", used_gb, total_gb, pct)),
                ]);
                disk_items.push(ListItem::new(line));
            }
            let disk_list = List::new(disk_items)
                .block(Block::default().title(" Disks ").borders(Borders::ALL));
            f.render_widget(disk_list, body_chunks[0]);

            // Процессы (Топ по памяти)
            let mut proc_list: Vec<_> = sys.processes().iter().collect();
            proc_list.sort_by(|a, b| b.1.memory().cmp(&a.1.memory()));
            let top_procs: Vec<Row> = proc_list
                .into_iter()
                .take(15)
                .map(|(pid, proc)| {
                    let mem_mb = proc.memory() as f64 / 1024.0 / 1024.0;
                    let cpu = proc.cpu_usage();
                    let name = proc.name().to_string_lossy().to_string();
                    Row::new(vec![
                        pid.to_string(),
                        name,
                        format!("{:.1} MB", mem_mb),
                        format!("{:.1}%", cpu),
                    ])
                })
                .collect();

            let proc_table = Table::new(
                top_procs,
                [
                    Constraint::Length(8),
                    Constraint::Percentage(50),
                    Constraint::Percentage(25),
                    Constraint::Percentage(15),
                ],
            )
            .header(
                Row::new(vec!["PID", "Name", "Memory", "CPU"])
                    .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            )
            .block(Block::default().title(" Top Processes (by RAM) ").borders(Borders::ALL));
            f.render_widget(proc_table, body_chunks[1]);

            // 4. Подвал
            let footer = Paragraph::new("Нажмите [q] или [Esc] для выхода | Обновление: каждые 500мс")
                .style(Style::default().fg(Color::DarkGray))
                .alignment(ratatui::layout::Alignment::Center)
                .block(Block::default().borders(Borders::ALL));
            f.render_widget(footer, chunks[3]);
        })?;

        // Обработка клавиш с тайм-аутом
        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => break,
                        _ => {}
                    }
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
    }

    // Восстановление терминала
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}
