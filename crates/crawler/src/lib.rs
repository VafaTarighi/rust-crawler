mod scraper;
mod get;
mod url;
mod parallel;

use std::collections::HashSet;

use scraper::Scraper;
use reqwest::Url;

pub struct ScraperBuilder {
    origin_url: String,
    thread_count: usize,
    depth: usize,
    host_only: bool
}

impl ScraperBuilder {
    pub fn new(origin_url: &str) -> Self {
        ScraperBuilder {
            origin_url: origin_url.to_string(), 
            thread_count: num_cpus::get(), 
            depth: std::usize::MAX,
            host_only: true
        }
    }

    pub fn threads(mut self, thread_count: usize) -> Self {
        self.thread_count = thread_count;
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
            thread_count: self.thread_count,
            visited: HashSet::new(),
            depth: self.depth,
            host_only: self.host_only
        })
    }

}


// costum error handling
#[derive(Debug)]
pub enum Error {
    Write { url: String, e: IoErr },
    Fetch { url: String, e: reqwest::Error },
    Build { url: String, e: String}
}

pub type Result<T> = std::result::Result<T, Error>;
type IoErr = std::io::Error;

impl<S: AsRef<str>> From<(S, IoErr)> for Error {
    fn from((url, e): (S, IoErr)) -> Self {
        Error::Write {
            url: url.as_ref().to_string(),
            e,
        }
    }
}

impl<S: AsRef<str>> From<(S, reqwest::Error)> for Error {
    fn from((url, e): (S, reqwest::Error)) -> Self {
        Error::Fetch {
            url: url.as_ref().to_string(),
            e,
        }
    }
}

impl<S: AsRef<str>> From<(S, String)> for Error {
    fn from((url, e): (S, String)) -> Self {
        Error::Build {
            url: url.as_ref().to_string(),
            e,
        }
    }
}



#[cfg(test)]
mod tests {
    use crate::ScraperBuilder;


    #[test]
    fn maintest() {
        let origin_url = "https://rolisz.ro/2020/03/01/web-crawler-in-rust/use";
        let threads = 8;
        let depth = 1;

        let mut scrpr = ScraperBuilder::new(origin_url)
            .threads(threads)
            .depth(depth)
            .build();

            match &mut scrpr {
                Err(e) => println!("Build Error: {:#?}", e),
                Ok(scrpr) => scrpr.start()
            }

    }
}
