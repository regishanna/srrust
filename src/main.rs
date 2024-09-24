use src_ogn::SrcOgn;
use std::{thread, time};

mod src_ogn;

fn main() {
    // lancement de la reception des trafic OGN
    SrcOgn::start_receive();

    // attente infinie
    loop {
        thread::sleep(time::Duration::from_secs(1));
    }
}
