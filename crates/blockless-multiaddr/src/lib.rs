#[macro_use]
mod macros;
mod parse;

pub use parse::*;

#[derive(Debug)]
pub struct Indicator<'a> {
    val: &'a [u8],
    begin: usize,
    end: usize,
}

#[derive(Debug)]
pub enum Error {
    Partial,
    InvalidToken,
}

#[derive(Debug)]
pub struct MultiAddr<'a> {
    paths: Vec<Indicator<'a>>,
}

impl<'a> Indicator<'a> {
    pub fn new(bs: &'a [u8], begin: usize, end: usize) -> Self {
        Self {
            val: &bs[begin..end],
            begin,
            end,
        }
    }

    pub fn value(&self) -> &[u8] {
        self.val
    }

    pub fn value_to_str(&self) -> &str {
        unsafe { std::str::from_utf8_unchecked(self.val) }
    }

    pub fn begin(&self) -> usize {
        self.begin
    }

    pub fn end(&self) -> usize {
        self.end
    }
}

impl<'a> MultiAddr<'a> {
    pub fn schema(&self) -> Result<&str, Error> {
        if !self.paths.is_empty() {
            std::str::from_utf8(self.paths[0].value()).map_err(|_| Error::InvalidToken)
        } else {
            Err(Error::Partial)
        }
    }

    pub fn parse(s: &[u8]) -> Result<MultiAddr, Error> {
        parse::parse(s)
    }

    pub fn paths_ref(&self) -> &Vec<Indicator<'a>> {
        &self.paths
    }

    pub fn to_url_string(&self) -> Result<String, Error> {
        let mut rs = String::new();
        let last = self.paths.len() - 1;
        for (i, indx) in self.paths.iter().enumerate() {
            rs += indx.value_to_str();
            if i == 0 {
                rs += "://";
            } else if i != last {
                rs += "/";
            }
        }
        Ok(rs)
    }
}
