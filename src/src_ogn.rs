use ureq;
use std::thread;

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
            let ogn_string = Self::get_ogn_string();
            println!("Reponse = {}", ogn_string);
        }
    }

    fn get_ogn_string() -> String {
        // tentative de lecture tant que ce n'est pas un succes
        loop {
            // on recupere les infos de trafic sur la france
            let response = match ureq::get("https://live.glidernet.org/lxml.php?a=0\
                                                           &b=51.3\
                                                           &c=42.1\
                                                           &d=8.4\
                                                           &e=-5.1")
                                               .call() {
                Ok(r) => r,
                _ => {
                    // TODO temporiser et faire une trace
                    continue;
                }
            };

            // on recupere le body de la reponse sous forme de chaine de caracteres
            match response.into_string() {
                Ok(s) => return s,
                _ => {
                    // TODO temporiser et faire une trace
                    continue;
                }
            }
        }
    }
}
