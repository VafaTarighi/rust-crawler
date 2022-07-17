use std::{collections::HashSet, time::Duration};
use std::time::Instant;
use reqwest::Url;

use crate::{url::filter_visited, parallel::FetchPool};



#[derive(Debug, PartialEq)]
pub struct Scraper {
    pub(crate) origin_url: Url,
    pub(crate) worker_count: usize,
    pub(crate) visited: HashSet<Url>,
    pub(crate) depth: usize,
    pub(crate) host_only: bool
}

impl Scraper {

    pub fn start(&mut self) {
        let now = Instant::now();


        let mut fpool = FetchPool::new(self.worker_count);

        // request for origin_url and fetch the page and
        let mut p = HashSet::new();
        p.insert(self.origin_url.clone());
        fpool.fetch(p);

        self.visited.insert(self.origin_url.to_owned());
        let page = fpool.get_wait();
        if let None = page {
            fpool.close();
            finished(now, 1);
            
            return;
        }
        
        let page = filter_visited(page.unwrap()[0].clone(), &self.visited, &self.origin_url, self.host_only);

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

            if let Some(mut results) = fpool.get_current(Some(Duration::from_millis(100))) {
                page_vec.append(&mut results);
            } else {
                break;
            }
        }

        fpool.close();

        
        finished(now, self.visited.len());
        
    }
}

fn finished(i: Instant, total_visited: usize) {
    println!("--------------------------------------------------FINISHED--------------------------------------------------");
    println!("\nVisited URLs: {}", total_visited);
    println!("elapsed time {}s", i.elapsed().as_secs());
}