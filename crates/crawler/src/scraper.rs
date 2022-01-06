use std::collections::HashSet;
use std::time::Instant;
use reqwest::Url;

use crate::{url::{filter_visited, filter_host}, parallel::FetchPool};



#[derive(Debug, PartialEq)]
pub struct Scraper {
    pub(crate) origin_url: Url,
    pub(crate) thread_count: usize,
    pub(crate) visited: HashSet<Url>,
    pub(crate) depth: usize,
    pub(crate) host_only: bool
}

impl Scraper {

    pub fn start(&mut self) {
        let now = Instant::now();


        // request for origin_url and fetch the page and
        let mut fpool = FetchPool::new(self.thread_count);
        fpool.fetch_single(&self.origin_url);

        self.visited.insert(self.origin_url.to_owned());

        let (found_urls, errors_str) = fpool.get_results();
        if errors_str != "Errors: []" {
            println!("{}", errors_str);
        }
        
        let mut new_urls = filter_visited(found_urls, &self.visited);
        if self.host_only {
            filter_host(&mut new_urls, &self.origin_url);
        }




        for _ in 0..self.depth {
            if new_urls.is_empty() {
                break;
            }

            fpool.fetch(new_urls.to_owned());

            self.visited.extend(new_urls);

            let (found_urls, errors_str) = fpool.get_results();
            println!("{}", errors_str);

            new_urls = filter_visited(found_urls, &self.visited);
            if self.host_only {
                filter_host(&mut new_urls, &self.origin_url);
            }

        }

        fpool.close();

        println!("--------------------------------------------------------------->");
        println!("\nVisited URLs: {}", self.visited.len());
        println!("elapsed time {}s", now.elapsed().as_secs());

        
    }
}