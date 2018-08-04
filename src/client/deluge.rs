pub type Result<T> = ::std::result::Result<T, Error>;

quick_error! {
    #[derive(Debug)]
    pub enum Error {}
}

#[derive(Debug)]
pub struct Deluge;

impl Deluge {
    pub fn new() -> Result<Self> {
        Ok(Deluge)
    }
}
