quick_error! {
    #[derive(Debug)]
    pub enum Error {}
}

#[derive(Debug)]
pub struct Deluge;

impl Deluge {
    pub fn new() -> Self {
        Deluge
    }
}
