use src_ogn::SrcOgn;
use src_adsbhub::SrcAdsbhub;
use std::io::Write;

mod traffic_infos;
mod dgramostream;
mod gdl90;
mod internal_com;
mod server;
mod client;
mod src_ogn;
mod src_adsbhub;

fn main() {
    // Init and customization of the trace system
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format(|buf, record| {
            let level_color = match record.level() {
                log::Level::Error => Some(anstyle::Color::from(anstyle::AnsiColor::Red)),
                log::Level::Warn => Some(anstyle::Color::from(anstyle::AnsiColor::Yellow)),
                _ => None
            };
            let level_style = anstyle::Style::new().fg_color(level_color);
            writeln!(
                buf,
                "[{}-{}{}{:#}-{}:{}] {}",
                chrono::Local::now().format("%H:%M:%S%.6f"),
                level_style,
                record.level(),
                level_style,
                record.file().unwrap_or("unknown"),
                record.line().unwrap_or(0),
                record.args()
            )
        })
        .init();

    log::info!("Start {} v{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));

    // Launch of reception of OGN traffic
    SrcOgn::start_receive();

    // Launch of reception of ADSBHub traffic
    SrcAdsbhub::start_receive();

    // Listening and processing client connections (blocking)
    server::listen_connections();
}
