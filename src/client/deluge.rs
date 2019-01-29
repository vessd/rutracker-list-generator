#[derive(Debug)]
pub struct Deluge;

impl Deluge {
    pub fn new() -> Result<Self, std::io::Error> {
        Ok(Deluge)
    }
}
