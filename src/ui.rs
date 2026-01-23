use ratatui::{
    prelude::*,
    symbols,
    widgets::{Axis, Block, Borders, Chart, Dataset, Gauge, GraphType, Paragraph, Sparkline, Wrap},
};

use crate::app::App;

pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // Main vertical layout: header, PSU panels, total power chart, footer
    let main_chunks = Layout::vertical([
        Constraint::Length(3),  // header
        Constraint::Min(26),   // PSU panels (includes per-PSU graphs)
        Constraint::Length(12), // total power chart
        Constraint::Length(1),  // footer
    ])
    .split(area);

    draw_header(frame, main_chunks[0], app);
    draw_psu_panels(frame, main_chunks[1], app);
    draw_power_graph(frame, main_chunks[2], app);
    draw_footer(frame, main_chunks[3], app);
}

fn draw_header(frame: &mut Frame, area: Rect, app: &App) {
    let total_power = app.total_power();
    let total_12v = app.total_12v_power();

    let uptime = if !app.readings.is_empty() {
        app.readings[0].uptime_hours
    } else {
        0.0
    };

    let mut header_text = format!(
        " Total: {:.0}W (12V: {:.0}W)    Uptime: {:.2}h   ",
        total_power, total_12v, uptime
    );

    for (i, reading) in app.readings.iter().enumerate() {
        header_text.push_str(&format!(" [PSU{}: {:.0}W]", i + 1, reading.input_power));
    }

    let header = Paragraph::new(Line::from(vec![
        Span::styled(header_text, Style::default().fg(Color::White).bold()),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(Span::styled(
                " corsair-top ",
                Style::default().fg(Color::Cyan).bold(),
            )),
    );

    frame.render_widget(header, area);
}

fn draw_psu_panels(frame: &mut Frame, area: Rect, app: &App) {
    let num_psus = app.readings.len();
    if num_psus == 0 {
        let msg = Paragraph::new("No PSUs detected. Waiting for data...")
            .style(Style::default().fg(Color::Yellow))
            .alignment(Alignment::Center);
        frame.render_widget(msg, area);
        return;
    }

    // Calculate max 12V pages across all PSUs for uniform section height
    let max_pages = app.readings.iter()
        .map(|r| r.twelve_v_pages.len())
        .max()
        .unwrap_or(0);

    // Split horizontally for each PSU
    let constraints: Vec<Constraint> = (0..num_psus)
        .map(|_| Constraint::Ratio(1, num_psus as u32))
        .collect();

    let psu_chunks = Layout::horizontal(constraints).split(area);

    for (i, reading) in app.readings.iter().enumerate() {
        draw_single_psu(frame, psu_chunks[i], reading, i, app, max_pages);
    }
}

fn draw_single_psu(frame: &mut Frame, area: Rect, reading: &crate::driver::PsuReadings, index: usize, app: &App, max_pages: usize) {
    let psu_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(Span::styled(
            format!(" PSU {} - {} (Bus {:03}:{:03}) ", index + 1, reading.name.trim_end_matches('\0').trim(), reading.bus, reading.address),
            Style::default().fg(Color::Cyan).bold(),
        ));

    let inner = psu_block.inner(area);
    frame.render_widget(psu_block, area);

    // Use max_pages across all PSUs so sections align vertically
    let pages_height = if max_pages == 0 {
        0u16
    } else {
        (max_pages as u16 + 1).saturating_add(2) // 1 header + pages + 2 borders
    };

    // Layout inside PSU panel: input, rails, 12V pages, temp/fan, power graph, temp graph
    let panel_chunks = Layout::vertical([
        Constraint::Length(5),            // input section
        Constraint::Length(5),            // rails section
        Constraint::Length(pages_height), // 12V pages (uniform across PSUs)
        Constraint::Length(2),            // temp and fan
        Constraint::Min(6),              // power sparkline
        Constraint::Length(6),            // temp sparkline
    ])
    .split(inner);

    draw_input_section(frame, panel_chunks[0], reading);
    draw_rails_section(frame, panel_chunks[1], reading);

    if max_pages > 0 {
        draw_12v_pages(frame, panel_chunks[2], reading);
    }

    draw_temp_fan(frame, panel_chunks[3], reading);
    draw_psu_power_sparkline(frame, panel_chunks[4], app, index);
    draw_psu_temp_sparkline(frame, panel_chunks[5], app, index);
}

fn draw_input_section(frame: &mut Frame, area: Rect, reading: &crate::driver::PsuReadings) {
    let power_ratio = (reading.input_power / 1600.0).min(1.0);

    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(" Input ", Style::default().fg(Color::Green)));

    let inner = input_block.inner(area);
    frame.render_widget(input_block, area);

    let input_chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(inner);

    let voltage_line = Line::from(vec![
        Span::styled("  Voltage ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{:>6.1}V", reading.input_voltage),
            Style::default().fg(Color::Green),
        ),
    ]);
    frame.render_widget(Paragraph::new(voltage_line), input_chunks[0]);

    let current_line = Line::from(vec![
        Span::styled("  Current ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{:>6.2}A", reading.input_current),
            Style::default().fg(Color::Green),
        ),
    ]);
    frame.render_widget(Paragraph::new(current_line), input_chunks[1]);

    // Power with gauge
    let power_color = if reading.input_power > 1200.0 {
        Color::Red
    } else if reading.input_power > 800.0 {
        Color::Yellow
    } else {
        Color::Green
    };

    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(power_color).bg(Color::DarkGray))
        .ratio(power_ratio)
        .label(format!("  Power {:>6.0}W", reading.input_power));

    frame.render_widget(gauge, input_chunks[2]);
}

fn draw_rails_section(frame: &mut Frame, area: Rect, reading: &crate::driver::PsuReadings) {
    let rail_names = ["12V ", " 5V ", "3.3V"];

    let rails_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(" Rails ", Style::default().fg(Color::Green)));

    let inner = rails_block.inner(area);
    frame.render_widget(rails_block, area);

    let rail_constraints: Vec<Constraint> = reading
        .rails
        .iter()
        .map(|_| Constraint::Length(1))
        .collect();

    let rail_chunks = Layout::vertical(rail_constraints).split(inner);

    for (i, rail) in reading.rails.iter().enumerate() {
        let name = if i < rail_names.len() {
            rail_names[i]
        } else {
            "????"
        };

        let rail_line = Line::from(vec![
            Span::styled(
                format!(" {} ", name),
                Style::default().fg(Color::Cyan).bold(),
            ),
            Span::styled(
                format!("{:>5.2}V ", rail.voltage),
                Style::default().fg(Color::Green),
            ),
            Span::styled(
                format!("{:>5.1}A ", rail.current),
                Style::default().fg(Color::White),
            ),
            Span::styled(
                format!("{:>5.0}W", rail.power),
                Style::default().fg(Color::Yellow),
            ),
        ]);
        frame.render_widget(Paragraph::new(rail_line), rail_chunks[i]);
    }
}

fn draw_12v_pages(frame: &mut Frame, area: Rect, reading: &crate::driver::PsuReadings) {
    if reading.twelve_v_pages.is_empty() {
        return;
    }

    let pages_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(" 12V Pages ", Style::default().fg(Color::Green)));

    let inner = pages_block.inner(area);
    frame.render_widget(pages_block, area);

    let mut lines: Vec<Line> = Vec::new();

    // Header line
    lines.push(Line::from(vec![
        Span::styled(
            " Pg  Volt   Curr   Power   OCP",
            Style::default().fg(Color::DarkGray),
        ),
    ]));

    for page in &reading.twelve_v_pages {
        let ocp_ratio = if page.ocp_limit > 0.0 {
            page.current / page.ocp_limit
        } else {
            0.0
        };

        let current_color = if ocp_ratio > 0.9 {
            Color::Red
        } else if ocp_ratio > 0.7 {
            Color::Yellow
        } else {
            Color::White
        };

        lines.push(Line::from(vec![
            Span::styled(
                format!(" {:>2} ", page.page),
                Style::default().fg(Color::Cyan),
            ),
            Span::styled(
                format!("{:>5.2}V ", page.voltage),
                Style::default().fg(Color::Green),
            ),
            Span::styled(
                format!("{:>5.1}A ", page.current),
                Style::default().fg(current_color),
            ),
            Span::styled(
                format!("{:>5.0}W ", page.power),
                Style::default().fg(Color::Yellow),
            ),
            Span::styled(
                format!("{:>5.0}A", page.ocp_limit),
                Style::default().fg(Color::DarkGray),
            ),
        ]));
    }

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}

fn draw_temp_fan(frame: &mut Frame, area: Rect, reading: &crate::driver::PsuReadings) {
    let temp1_color = temp_color(reading.temp1);
    let temp2_color = temp_color(reading.temp2);

    let fan_color = if reading.fan_speed > 0.0 {
        Color::Green
    } else {
        Color::DarkGray
    };

    let info_chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(area);

    let temp_line = Line::from(vec![
        Span::styled(" Temp: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{:.1}", reading.temp1),
            Style::default().fg(temp1_color),
        ),
        Span::styled("\u{00b0}C", Style::default().fg(temp1_color)),
        Span::styled(" / ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{:.1}", reading.temp2),
            Style::default().fg(temp2_color),
        ),
        Span::styled("\u{00b0}C", Style::default().fg(temp2_color)),
    ]);
    frame.render_widget(Paragraph::new(temp_line), info_chunks[0]);

    let fan_line = Line::from(vec![
        Span::styled(" Fan:  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{:.0} RPM", reading.fan_speed),
            Style::default().fg(fan_color),
        ),
    ]);
    frame.render_widget(Paragraph::new(fan_line), info_chunks[1]);
}

fn draw_psu_power_sparkline(frame: &mut Frame, area: Rect, app: &App, index: usize) {
    if index >= app.power_history.len() || app.power_history[index].is_empty() {
        return;
    }

    let data: Vec<u64> = app.power_history[index].iter().map(|v| *v as u64).collect();
    let current = if index < app.readings.len() { app.readings[index].input_power } else { 0.0 };
    let max_val = app.power_history[index].iter().cloned().fold(100.0_f64, f64::max) as u64;

    let sparkline = Sparkline::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(Span::styled(
                    format!(" {:.0}W ", current),
                    Style::default().fg(Color::Green),
                )),
        )
        .data(&data)
        .max(if app.is_ax1600i { 1600 } else { max_val.max(100) + 50 })
        .style(Style::default().fg(Color::Green));

    frame.render_widget(sparkline, area);
}

fn draw_psu_temp_sparkline(frame: &mut Frame, area: Rect, app: &App, index: usize) {
    if index >= app.temp_history.len() || app.temp_history[index].is_empty() {
        return;
    }

    let data: Vec<u64> = app.temp_history[index].iter().map(|v| *v as u64).collect();
    let current = if index < app.readings.len() { app.readings[index].temp1 } else { 0.0 };
    let color = temp_color(current);

    let sparkline = Sparkline::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(Span::styled(
                    format!(" {:.1}\u{00b0}C ", current),
                    Style::default().fg(color),
                )),
        )
        .data(&data)
        .max(80)
        .style(Style::default().fg(color));

    frame.render_widget(sparkline, area);
}

fn draw_power_graph(frame: &mut Frame, area: Rect, app: &App) {
    if app.total_power_history.is_empty() {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(Span::styled(" Total Power (W) ", Style::default().fg(Color::Cyan).bold()));
        frame.render_widget(block, area);
        return;
    }

    let num_psus = app.readings.len().max(1) as f64;

    // Scale thresholds by PSU count
    let max_w = 1600.0 * num_psus;
    let warn_w = 1500.0 * num_psus;
    let ref_w = 1000.0 * num_psus;

    let max_y = if app.is_ax1600i {
        max_w + 100.0
    } else {
        let peak = app.total_power_history.iter().cloned().fold(100.0_f64, f64::max);
        (peak * 1.2).max(200.0)
    };

    let x_len = 59.0_f64; // fixed x range for consistent scaling
    let history_len = app.total_power_history.len();
    let x_offset = if history_len as f64 > x_len { history_len as f64 - x_len } else { 0.0 };

    let power_data: Vec<(f64, f64)> = app
        .total_power_history
        .iter()
        .enumerate()
        .map(|(i, v)| (i as f64 - x_offset, *v))
        .collect();

    let x_max = x_len;

    let mut datasets = vec![
        Dataset::default()
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(Color::Green))
            .data(&power_data),
    ];

    // Reference lines for AX1600i (scaled by PSU count)
    let ref_max: Vec<(f64, f64)>;
    let ref_warn: Vec<(f64, f64)>;
    let ref_mid: Vec<(f64, f64)>;

    if app.is_ax1600i {
        ref_max = vec![(0.0, max_w), (x_max, max_w)];
        ref_warn = vec![(0.0, warn_w), (x_max, warn_w)];
        ref_mid = vec![(0.0, ref_w), (x_max, ref_w)];

        datasets.push(
            Dataset::default()
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(Color::Red))
                .data(&ref_max),
        );
        datasets.push(
            Dataset::default()
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(Color::Yellow))
                .data(&ref_warn),
        );
        datasets.push(
            Dataset::default()
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(Color::DarkGray))
                .data(&ref_mid),
        );
    }

    let y_labels = if app.is_ax1600i {
        vec![
            Span::styled("0", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:.0}", ref_w), Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:.0}", warn_w), Style::default().fg(Color::Yellow)),
            Span::styled(format!("{:.0}", max_w), Style::default().fg(Color::Red)),
        ]
    } else {
        vec![
            Span::styled("0", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:.0}", max_y / 2.0), Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:.0}", max_y), Style::default().fg(Color::DarkGray)),
        ]
    };

    let current_label = format!(" Total Power: {:.0}W ", app.total_power());

    let chart = Chart::new(datasets)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(Span::styled(current_label, Style::default().fg(Color::Cyan).bold())),
        )
        .x_axis(
            Axis::default()
                .bounds([0.0, x_max])
                .labels(vec![Span::raw("")]),
        )
        .y_axis(
            Axis::default()
                .bounds([0.0, max_y])
                .labels(y_labels),
        );

    frame.render_widget(chart, area);
}


fn draw_footer(frame: &mut Frame, area: Rect, app: &App) {
    let rate_str = if app.tick_rate_ms >= 1000 {
        format!("{:.1}s", app.tick_rate_ms as f64 / 1000.0)
    } else {
        format!("{}ms", app.tick_rate_ms)
    };

    let footer = Paragraph::new(Line::from(vec![
        Span::styled("[q]", Style::default().fg(Color::Cyan).bold()),
        Span::styled(" Quit  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[+/-]", Style::default().fg(Color::Cyan).bold()),
        Span::styled(format!(" Poll: {}  ", rate_str), Style::default().fg(Color::DarkGray)),
    ]))
    .alignment(Alignment::Center);

    frame.render_widget(footer, area);
}

fn temp_color(temp: f64) -> Color {
    if temp > 60.0 {
        Color::Red
    } else if temp > 50.0 {
        Color::Yellow
    } else {
        Color::Green
    }
}
