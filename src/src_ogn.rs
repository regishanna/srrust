use ureq;
use std::{thread, time};

pub struct SrcOgn {}

impl SrcOgn {
    /// Lance la reception des trafics OGN
    pub fn start_receive() {
        thread::spawn(|| {
            Self::work_thread();
        });
    }

    fn new() -> SrcOgn {
        SrcOgn {}
    }

    fn work_thread() {
        let ogn = Self::new();
        loop {
            match Self::get_and_send_positions() {
                Err(e) => log::warn!("{:?}", e),
                Ok(()) => ()
            }
            thread::sleep(time::Duration::from_secs(5));
        }
    }

    fn get_and_send_positions() -> anyhow::Result<()> {
        let ogn_string = Self::get_ogn_string()?;
        println!("{}", ogn_string);
        Ok(())
    }

    fn get_ogn_string() -> anyhow::Result<String> {
        // On recupere les infos de trafic sur la france
        let ogn_string = ureq::get("https://live.glidernet.org/lxml.php?a=0\
                                                  &b=51.3\
                                                  &c=42.1\
                                                  &d=8.4\
                                                  &e=-5.1")
                                      .call()?
                                      .into_string()?;
        Ok(ogn_string)
    }

}