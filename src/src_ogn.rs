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
            
        }
    }
}
