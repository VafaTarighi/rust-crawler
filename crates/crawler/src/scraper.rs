use std::collections::HashSet;
use std::thread;
use std::time::Instant;
use std::sync::mpsc;

use reqwest::Url;

use crate::fetcher::GetHandle;
use crate::parallel::ReqMsg;
use crate::parallel::RspMsg;
use crate::url::filter_visited;
use crate::url::filter_host;
use crate::url::get_links_from_html;
use crate::url::write_to_file;


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
        let fetcher = GetHandle::new();
        let fetch = fetcher.get_url(self.origin_url.clone())
            .map_err(|e| (&self.origin_url, e)).unwrap();
        
        self.visited.insert(self.origin_url.clone());

        let found_urls = get_links_from_html(&fetch);
        let mut new_urls = filter_visited(found_urls, &self.visited);

        write_to_file(fetch)
            .or_else::<(), _>(|e| {
                Ok(println!("{:#?}", e))
            }).unwrap();
        // let mut new_urls = HashSet::new();
        // new_urls.insert(self.origin_url.to_owned());
        
        let mut t_handlers = vec![];
        let mut chnls = vec![];

        

        // spawn threads
        for _ in 0..self.thread_count {
            let fetcher = GetHandle::new();
            let (url_tx, url_rx) = mpsc::channel();
            let (f_tx, f_rx) = mpsc::channel();
            let handler = thread::spawn(move || {
                loop {
                    f_tx.send(RspMsg::READY).unwrap();

                    let msg: ReqMsg = url_rx.recv().unwrap();
                    match msg {
                        ReqMsg::EXIT => break,

                        ReqMsg::FETCH(url) => {
                            let fetch = fetcher.get_url(url);
                            // if fetch failes: send error and continue
                            if fetch.is_err() {
                                f_tx.send(RspMsg::RESPOND(HashSet::new(), fetch.err())).unwrap();
                                continue;
                            }

                            let fetch = fetch.unwrap();

                            if fetch.is_html() {
                                let found_urls = get_links_from_html(&fetch);
                                println!("Visited: {} found {} links", fetch.get_url(), found_urls.len());
                                f_tx.send(RspMsg::RESPOND(found_urls, None)).unwrap();
                            }

                            let write_res = write_to_file(fetch);
                            // if write failes: send error and continue
                            if write_res.is_err() {
                                f_tx.send(RspMsg::RESPOND(HashSet::new(), write_res.err())).unwrap();
                            }
                            
                        }
                    }
                }
            });

            t_handlers.push(handler);
            chnls.push((url_tx, f_rx));
        }

        let mut chnl_itr = chnls.iter().cycle();

        let mut errors = vec![];

        for _ in 0..self.depth {

            let mut found_urls = HashSet::new();

            for url in new_urls.into_iter() {
                while let Some((tx, rx)) = chnl_itr.next() {
                    match rx.try_recv() {
                        Ok(msg) => {
                            match msg {
                                RspMsg::RESPOND(urls, e) => {
                                    found_urls.extend(urls);
                                    if let Some(e) = e {
                                        errors.push(e);
                                    }
                                },
                                RspMsg::READY => {
                                    tx.send(ReqMsg::FETCH(url.to_owned())).unwrap();
                                    self.visited.insert(url);
                                    break;
                                }
                            }
                        },
                        Err(e) => {
                            match e {
                                mpsc::TryRecvError::Empty => continue,
                                mpsc::TryRecvError::Disconnected => panic!("{}", e)
                            }
                        }
                    }
                }
            }

        
            new_urls = filter_visited(found_urls, &self.visited);
            if self.host_only {
                filter_host(&mut new_urls, &self.origin_url);
            }
            

            if new_urls.is_empty() {
                break;
            }

        }

        // evacuate channels
        chnls.iter()
            .for_each(|(_, rx) | {
                match rx.recv() {
                    Ok(msg) => {
                        match msg {
                            RspMsg::RESPOND(_, e) => {
                                if let Some(e) = e {
                                    errors.push(e);
                                }
                            },
                            _ => ()
                        }
                    },
                    Err(e) => panic!("{}", e)
                }
            });
        
        println!(
            "Errors: {:#?}",
            errors
        );

        println!("Visited URLs: {}", self.visited.len());
        println!("\nelapsed time {}s", now.elapsed().as_secs());

        chnls.iter()
            .for_each(|(tx, _)| tx.send(ReqMsg::EXIT).unwrap());
        
        for jh in t_handlers {
            jh.join().unwrap();
        }
    }
}