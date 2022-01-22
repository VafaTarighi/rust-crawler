use std::{collections::HashSet, thread::{self, JoinHandle}};

use crossbeam_channel::{Sender, Receiver};
use reqwest::Url;

use crate::{get::GetHandle, Error, url::{get_links_from_html, write_to_file}};


pub(crate) enum ReqMsg {
    FETCH(Url),
    EXIT,
}

pub(crate) enum RspMsg {
    PAGE(HashSet<Url>),
    ERROR(Error),
    FIN
}

pub struct FetchPool {
    t_count: usize,
    url_tx: Sender<ReqMsg>,
    fetch_rx: Receiver<RspMsg>,
    ft_handles: Vec<FetchThreadHandle>,
    work: usize
}

impl FetchPool {
    pub(crate) fn new(t_count: usize) -> Self {

        let mut ft_handles = vec![];

        let (url_tx, url_rx) = crossbeam_channel::unbounded();
        let (fetch_tx, fetch_rx) = crossbeam_channel::unbounded();

        // spawn threads
        for _ in 0..t_count {

            ft_handles.push(
                FetchThread::spawn(fetch_tx.clone(), url_rx.clone())
            );

        }

        Self {
            t_count,
            url_tx,
            fetch_rx,
            ft_handles,
            work: 0
        }

    }

    fn recv(&self) -> RspMsg {
        self.fetch_rx.recv().unwrap()
    }

    #[allow(dead_code)]
    pub(crate) fn t_count(&self) -> usize {
        self.t_count
    }

    pub(crate) fn fetch_single(&mut self, url: &Url) {
        self.work += 1;
        self.url_tx.send(ReqMsg::FETCH(url.to_owned())).unwrap();
    }

    pub(crate) fn fetch(&mut self, page: HashSet<Url>) {
        self.work += page.len();

        page.iter()
            .for_each(|url| {
                self.fetch_single(url)
            });
    }

    pub(crate) fn fetch_current(&mut self) -> Option<(Vec<HashSet<Url>>, Vec<Error>)> {
        if self.work == 0 {
            return None
        }
        let mut pages = vec![];
        let mut errors = vec![];
        for msg in self.fetch_rx.try_iter() {
            match msg {
                RspMsg::PAGE(page) => {
                    self.work -= 1;
                    pages.push(page)
                },
                RspMsg::FIN => self.work -= 1,
                RspMsg::ERROR(e) => errors.push(e)
            }
        }
        
        Some((pages, errors))
    }

    pub(crate) fn get_page(&mut self) -> Option<(HashSet<Url>, Option<Error>)> {
        if self.work == 0 {
            return None
        }
        let msg = self.recv();
        self.work -= 1;

        match msg {
            RspMsg::PAGE(page) => Some((page, None)),
            RspMsg::FIN => Some((HashSet::new(), None)),
            RspMsg::ERROR(e) => Some((HashSet::new(), Some(e)))
        }

    }

    pub(crate) fn close(mut self) {

        for _ in 0..self.t_count {
            self.url_tx.send(ReqMsg::EXIT).unwrap();
        }

        let vc = vec![];
        let fths = self.ft_handles;
        self.ft_handles = vc;
        fths.into_iter()
            .for_each(|fth| {
                fth.kill()
            })
    }
    

}

struct FetchThread;

impl FetchThread {
    fn spawn(fetch_tx: Sender<RspMsg>, url_rx: Receiver<ReqMsg>) -> FetchThreadHandle {
        let get_handle = GetHandle::new();

        let mth = MasterThreadHandle {
            fetch_tx,
            url_rx
        };

        let handle = thread::spawn(move || {

            loop {
                // to_master.send(RspMsg::READY).unwrap();
                
                match mth.receive() {

                    ReqMsg::EXIT => break,

                    ReqMsg::FETCH(url) => {
                        
                        // fetch data from url
                        let fetch = get_handle.get_url(url);
                        if fetch.is_err() {
                            mth.send_err(fetch.unwrap_err());
                            mth.send_fin();
                            continue;
                        }

                        let fetch = fetch.unwrap();

                        if fetch.is_html() {
                            let found_urls = get_links_from_html(&fetch);
                            println!("Visited: {} found {} links", fetch.get_url(), found_urls.len());
                            mth.send_urls(found_urls);
                        } else {
                            mth.send_fin();
                        }

                        let write_res = write_to_file(fetch);
                        // if write failes: send error and continue
                        if write_res.is_err() {
                            mth.send_err(
                                Error::from(write_res.unwrap_err())
                            )
                        }
                    }
                }
            }
        });

        FetchThreadHandle(handle)
    }

}


struct FetchThreadHandle(JoinHandle<()>);

impl FetchThreadHandle {
    fn kill(self) {
        self.0.join().unwrap();
    }
}


struct MasterThreadHandle {
    fetch_tx: Sender<RspMsg>,
    url_rx: Receiver<ReqMsg>
}

impl MasterThreadHandle {

    fn send_urls(&self, found_urls: HashSet<Url>) {
        self.fetch_tx.send(RspMsg::PAGE(found_urls)).unwrap();
    }

    fn send_fin(&self) {
        self.fetch_tx.send(RspMsg::FIN).unwrap();
    }

    fn send_err(&self, e: Error) {
        self.fetch_tx.send(RspMsg::ERROR(e)).unwrap();
    }

    fn receive(&self) -> ReqMsg {
        self.url_rx.recv().unwrap()
    }
}

