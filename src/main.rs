use src_ogn::SrcOgn;
use std::io::Write;

mod traffic_infos;
mod internal_com;
mod server;
mod client;
mod src_ogn;

fn main() {
    // Init et personnalisation du systeme de traces
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
                record.file().unwrap_or("inconnu"),
                record.line().unwrap_or(0),
                record.args()
            )
        })
        .init();

    log::info!("Lancement {} v{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));

    // Lancement de la reception des trafic OGN
    SrcOgn::start_receive();

    // Ecoute et traitement des connexions des clients (bloquant)
    server::listen_connections();
}
