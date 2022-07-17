mod scraper;
mod get;
mod url;
mod parallel;

use std::{collections::HashSet, fmt::Display};

use scraper::Scraper;
use reqwest::Url;

pub struct ScraperBuilder {
    origin_url: String,
    worker_count: usize,
    depth: usize,
    host_only: bool
}

impl ScraperBuilder {
    pub fn new(origin_url: &str) -> Self {
        ScraperBuilder {
            origin_url: origin_url.to_string(), 
            worker_count: num_cpus::get(), 
            depth: std::usize::MAX,
            host_only: false
        }
    }

    pub fn workers(mut self, thread_count: usize) -> Self {
        self.worker_count = thread_count;
        self
    }

    pub fn depth(mut self, depth: usize) -> Self {
        self.depth = depth;
        self
    }

    pub fn host_only(mut self, yes: bool) -> Self {
        self.host_only = yes;
        self
    }

    pub fn build(&self) -> Result<Scraper> {
        let origin_url = Url::parse(&self.origin_url)
            .map_err(|e| (&self.origin_url, e.to_string()))?;

        Ok(Scraper {
            origin_url: origin_url,
            worker_count: self.worker_count,
            visited: HashSet::new(),
            depth: self.depth,
            host_only: self.host_only
        })
    }

}


// costum error handling
#[derive(Debug)]
pub enum Error {
    Write { url: Url, e: IoErr },
    Fetch { url: Url, e: reqwest::Error },
    Scraper { msg: String, e: String}
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Write {url, e} => write!(f, "ERROR: Write {{\n\t{}\n\t{:#?}\n}}",url, e),
            Self::Fetch {url, e} => write!(f, "ERROR: Fetch {{\n\t{}\n\t{:#?}\n}}",url, e),
            Self::Scraper {msg, e} => write!(f, "ERROR: Scraper {{\n\t{}\n\t{:#?}\n}}",msg, e)
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
type IoErr = std::io::Error;

impl From<(Url, IoErr)> for Error {
    fn from((url, e): (Url, IoErr)) -> Self {
        Error::Write {
            url,
            e
        }
    }
}

impl From<(Url, reqwest::Error)> for Error {
    fn from((url, e): (Url, reqwest::Error)) -> Self {
        Error::Fetch {
            url,
            e,
        }
    }
}

impl<S: AsRef<str>> From<(S, String)> for Error {
    fn from((msg, e): (S, String)) -> Self {
        Error::Scraper {
            msg: msg.as_ref().to_string(),
            e,
        }
    }
}



#[cfg(test)]
mod tests {
    use crate::ScraperBuilder;


    #[test]
    fn maintest() {
        let origin_url = "https://rolisz.ro/";
        let workers = 8;
        let depth = 2;

        let mut scrpr = ScraperBuilder::new(origin_url)
            .workers(workers)
            .depth(depth)
            .build();

            match &mut scrpr {
                Err(e) => println!("{}", e),
                Ok(scrpr) => scrpr.start()
            }

    }
}
