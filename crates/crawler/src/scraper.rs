use std::collections::HashSet;
use std::time::Instant;
use reqwest::Url;

use crate::{url::filter_visited, parallel::FetchPool};



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

        let (page, error) = fpool.get_page().unwrap();
        if let Some(e) = error {
            println!("{:#?}", e);
        }
        
        let page = filter_visited(page, &self.visited, &self.origin_url, self.host_only);

        let mut page_count = 0;
        let mut  page_vec = vec![];
        page_vec.push(page);

        while page_count <= self.depth {


            while !page_vec.is_empty() {

                let mut page = page_vec.pop().unwrap();
                page = filter_visited(page, &self.visited, &self.origin_url, self.host_only);
                self.visited.extend(page.clone());

                fpool.fetch(page);
                page_count += 1;
                if page_count > self.depth {
                    break;
                }
            }

            if page_count > self.depth {
                break;
            }

            if let Some(mut results) = fpool.get_current() {
                page_vec.append(&mut results.0);
                if !results.1.is_empty() {
                    println!("Errors: {:#?}", results.1);
                }
            } else {
                break;
            }
        }

        loop {
            if let Some(results) = fpool.get_current() {
                if !results.1.is_empty() {
                    println!("Errors: {:#?}", results.1);
                }
            } else {
                break;
            }
        }

        fpool.close();

        println!("--------------------------------------------------FINISHED--------------------------------------------------");
        println!("\nVisited URLs: {}", self.visited.len());
        println!("elapsed time {}s", now.elapsed().as_secs());

        
    }
}